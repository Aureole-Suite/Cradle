use gospel::write::{Label, Le as _, Writer};
use snafu::prelude::*;

use crate::permute;

use super::{abbr::*, ImageData, Itp, ItpStatus, Palette};

#[derive(Debug, Snafu)]
pub enum Error {
	#[allow(private_interfaces)]
	#[snafu(context(false))]
	Invalid {
		source: InnerError,
		backtrace: std::backtrace::Backtrace,
	},
}

#[derive(Debug, Snafu)]
#[snafu(module(e), context(suffix(false)))]
enum InnerError {
	#[snafu(context(false))]
	Write { source: gospel::write::Error },

	#[snafu(display("the specified revision cannot represent this file"))]
	Unrepresentable,

	#[snafu(display("the specified format does not support external palettes"))]
	ExternalPalette,

	#[snafu(display("CCPI can only store indexed images"))]
	CcpiMustBeIndexed,

	#[snafu(display("CCPI only supports Bz_1 compression or none"))]
	CcpiCompression,

	#[snafu(display("CCPI does not support mipmaps"))]
	CcpiMipmaps,

	#[snafu(display("AFastMode2 can only store 16 colors per 8×16 tile"))]
	AFastMode2Colors,

	#[snafu(display("{what} is not yet implemented"))]
	Todo { what: String },
}

impl From<gospel::write::Error> for Error {
	fn from(source: gospel::write::Error) -> Self {
		InnerError::from(source).into()
	}
}

macro_rules! bail {
	($e:expr) => {
		$e.fail::<!>()?
	};
}

pub fn write(itp: &Itp) -> Result<Vec<u8>, Error> {
	let Itp {
		ref status,
		width,
		height,
		ref data,
	} = *itp;

	let Some(head) = (match status.itp_revision {
		IR::V1 => status_to_flags(status).and_then(flags_to_gen1),
		IR::V2 => status_to_flags(status),
		IR::V3 => return write_revision_3(itp),
	}) else {
		bail!(e::Unrepresentable);
	};

	let mut f = Writer::new();
	f.u32(head);

	if status.base_format == BFT::Indexed3 {
		f.slice(&write_ccpi(itp)?);
	} else {
		f.u32(width);
		f.u32(height);

		if let ImageData::Indexed(pal, _) = data {
			let fixed_size = matches!(head, 1000 | 1002);
			let (is_external, pal_size, pal) = write_ipal(status, pal, fixed_size)?;
			ensure!(!is_external, e::ExternalPalette);
			if !fixed_size {
				f.u32(pal_size as u32);
			};
			f.slice(&pal);
		}

		for (width, height, range) in super::mipmaps(width, height, data.pixel_count()) {
			f.slice(&write_idat(status, width, height, data, range)?);
		}
	}
	Ok(f.finish()?)
}

fn write_revision_3(itp: &Itp) -> Result<Vec<u8>, Error> {
	fn chunk(f: &mut Writer, fourcc: &[u8; 4], body: Writer) {
		f.slice(fourcc);
		f.u32(body.len() as u32);
		f.append(body)
	}

	let Itp {
		ref status,
		width,
		height,
		ref data,
	} = *itp;

	let start = Label::new();
	let end = Label::new();

	let mut f = Writer::new();
	f.place(start);
	f.slice(b"ITP\xFF");

	chunk(&mut f, b"IHDR", {
		let mut f = Writer::new();
		f.u32(32);
		f.u32(width);
		f.u32(height);
		f.diff32(start, end);
		f.u16(status.itp_revision as u16);
		f.u16(status.base_format as u16);
		f.u16(status.pixel_format as u16);
		f.u16(status.pixel_bit_format as u16);
		f.u16(status.compression as u16);
		f.u16(status.multi_plane as u16);
		f.u32(0);
		f
	});

	chunk(&mut f, b"IMIP", {
		let mut f = Writer::new();
		let nmip = super::mipmaps(width, height, data.pixel_count()).count();
		f.u32(12);
		f.u16(status.mipmap as u16);
		f.u16((nmip - 1) as u16);
		f.u32(0);
		f
	});

	// IHAS: ignored

	if let ImageData::Indexed(pal, _) = data {
		chunk(&mut f, b"IPAL", {
			let mut f = Writer::new();
			let (is_external, pal_size, pal) = write_ipal(status, pal, false)?;
			f.u32(8);
			f.u16(is_external as u16);
			f.u16(pal_size as u16);
			f.slice(&pal);
			f
		});
	}

	if let Some(use_alpha) = status.use_alpha {
		chunk(&mut f, b"IALP", {
			let mut f = Writer::new();
			f.u32(8);
			f.u16(use_alpha as u16);
			f.u16(0);
			f
		});
	}

	for (n, (width, height, range)) in super::mipmaps(width, height, data.pixel_count()).enumerate()
	{
		chunk(&mut f, b"IDAT", {
			let mut f = Writer::new();
			f.u32(8);
			f.u16(0);
			f.u16(n as u16);
			f.slice(&write_idat(status, width, height, data, range)?);
			f
		});
	}

	chunk(&mut f, b"IEND", Writer::new());

	f.place(end);
	Ok(f.finish()?)
}

pub fn status_to_flags(status: &ItpStatus) -> Option<u32> {
	let mut flags = 0;

	macro_rules! bits {
		($($bit:expr),*) => {
			{ $(flags |= 1<<$bit;)* }
		}
	}

	match status.itp_revision {
		IR::V1 => bits!(),
		IR::V2 => bits!(30),
		IR::V3 => return None,
	}

	match (status.base_format, status.pixel_bit_format) {
		(BFT::Indexed1, PBFT::Indexed) => bits!(0, 20),
		(BFT::Indexed2, PBFT::Indexed) => bits!(0, 21),
		(BFT::Indexed3, PBFT::Indexed) => bits!(0, 22),
		(BFT::Argb16, PBFT::Argb16_1) => bits!(3),
		(BFT::Argb16, PBFT::Argb16_2) => bits!(1),
		(BFT::Argb16, PBFT::Argb16_3) => bits!(2),
		(BFT::Argb32, PBFT::Argb32) => bits!(4, 20),
		(BFT::Bc1, PBFT::Compressed) => bits!(24),
		(BFT::Bc2, PBFT::Compressed) => bits!(25),
		(BFT::Bc3, PBFT::Compressed) => bits!(26),
		_ => return None,
	}

	if status.base_format != BFT::Indexed3 {
		match status.compression {
			CT::None => bits!(15),
			CT::Bz_1 => bits!(16),
			CT::Bz_2 => bits!(16, 17),
			CT::C77 => return None,
		}
	}

	match status.pixel_format {
		PFT::Linear => bits!(11),
		PFT::Pfp_1 => bits!(10),
		PFT::Pfp_2 => bits!(12),
		PFT::Pfp_3 => bits!(13),
		PFT::Pfp_4 => bits!(14),
	}

	match status.multi_plane {
		MPT::None => bits!(),
	}

	match status.mipmap {
		MT::None => bits!(),
		MT::Mipmap_1 => return None,
		MT::Mipmap_2 => return None,
	}

	match status.use_alpha {
		Some(true) => bits!(28),
		Some(false) => bits!(29),
		None => bits!(),
	}

	Some(flags)
}

pub fn flags_to_gen1(flags: u32) -> Option<u32> {
	Some(match flags {
		0x108802 => 999,  // Argb16_2, None, Linear
		0x108801 => 1000, // Indexed1, None, Linear
		0x110802 => 1001, // Argb16_2, Bz_1, Linear
		0x110801 => 1002, // Indexed1, Bz_1, Linear
		0x110402 => 1003, // Argb16_2, Bz_1, Pfp_1
		0x110401 => 1004, // Indexed1, Bz_1, Pfp_1
		0x210401 => 1005, // Indexed2, Bz_1, Pfp_1
		0x400401 => 1006, // Indexed3, Ccpi, Pfp_1
		_ => return None,
	})
}

fn write_ipal(
	status: &ItpStatus,
	pal: &Palette,
	fixed_size: bool,
) -> Result<(bool, usize, Vec<u8>), Error> {
	match pal {
		Palette::Embedded(pal) => {
			let mut colors = pal.to_owned();
			for c in &mut colors {
				let [b, g, r, a] = u32::to_le_bytes(*c);
				*c = u32::from_le_bytes([r, g, b, a]);
			}
			if status.base_format == BFT::Indexed2 {
				for i in (1..colors.len()).rev() {
					colors[i] = colors[i].wrapping_sub(colors[i - 1])
				}
			}

			if fixed_size {
				colors.resize(256, 0);
			}

			let mut f = Writer::new();
			for color in colors {
				f.u32(color);
			}

			Ok((
				false,
				pal.len(),
				maybe_compress(status.compression, &f.finish()?),
			))
		}
		Palette::External(path) => Ok((true, 0, path.to_bytes_with_nul().to_owned())),
	}
}

fn write_idat(
	status: &ItpStatus,
	width: u32,
	height: u32,
	data: &ImageData,
	range: std::ops::Range<usize>,
) -> Result<Vec<u8>, Error> {
	let bc_range = range.start / 16..range.end / 16;
	Ok(match data {
		ImageData::Indexed(_, data) => match status.base_format {
			BFT::Indexed1 => {
				write_idat_simple(&data[range], status, width, height, u8::to_le_bytes)
			}
			BFT::Indexed2 => {
				let data = a_fast_mode2(&data[range], width, height)?;
				let mut f = Writer::new();
				f.u32(data.len() as u32);
				f.slice(&maybe_compress(status.compression, &data));
				f.finish()?
			}
			BFT::Indexed3 => bail!(e::Todo {
				what: "CCPI is not supported for revision 3"
			}),
			_ => unreachable!(),
		},
		ImageData::Argb16(_, data) => {
			write_idat_simple(&data[range], status, width, height, u16::to_le_bytes)
		}
		ImageData::Argb32(data) => {
			write_idat_simple(&data[range], status, width, height, u32::to_le_bytes)
		}
		ImageData::Bc1(data) => write_idat_simple(
			&data[bc_range],
			status,
			width / 4,
			height / 4,
			u64::to_le_bytes,
		),
		ImageData::Bc2(data) => write_idat_simple(
			&data[bc_range],
			status,
			width / 4,
			height / 4,
			u128::to_le_bytes,
		),
		ImageData::Bc3(data) => write_idat_simple(
			&data[bc_range],
			status,
			width / 4,
			height / 4,
			u128::to_le_bytes,
		),
		ImageData::Bc7(data) => write_idat_simple(
			&data[bc_range],
			status,
			width / 4,
			height / 4,
			u128::to_le_bytes,
		),
	})
}

fn write_idat_simple<T: Clone, const N: usize>(
	data: &[T],
	status: &ItpStatus,
	width: u32,
	height: u32,
	to_le_bytes: fn(T) -> [u8; N],
) -> Vec<u8> {
	let data = do_swizzle(
		data.to_vec(),
		width as usize,
		height as usize,
		status.pixel_format,
	);
	let data = data.into_iter().flat_map(to_le_bytes).collect::<Vec<u8>>();
	maybe_compress(status.compression, &data)
}

fn do_swizzle<T>(mut data: Vec<T>, width: usize, height: usize, pixel_format: PFT) -> Vec<T> {
	match pixel_format {
		PFT::Linear => {}
		PFT::Pfp_1 => permute::swizzle(&mut data, height, width, 8, 16),
		PFT::Pfp_2 => permute::swizzle(&mut data, height, width, 32, 32),
		PFT::Pfp_3 => permute::morton(&mut data, height, width),
		PFT::Pfp_4 => {
			permute::swizzle(&mut data, height, width, 8, 1);
			permute::morton(&mut data, width * height / 8, 8);
		}
	}
	data
}

fn write_ccpi(itp: &Itp) -> Result<Vec<u8>, Error> {
	let ImageData::Indexed(pal, pixels) = &itp.data else {
		bail!(e::CcpiMustBeIndexed)
	};

	ensure!(
		pixels.len() == itp.width as usize * itp.height as usize,
		e::CcpiMipmaps
	);

	let mut status_copy = itp.status.clone();
	status_copy.compression = CT::None;

	let mut g = Writer::new();
	let mut flags = 0;
	let (external, pal_size, pal) = write_ipal(&status_copy, pal, false)?;
	if external {
		flags |= 1 << 9;
	}

	match itp.status.compression {
		CT::None => {}
		CT::Bz_1 => flags |= 1 << 15,
		CT::Bz_2 | CT::C77 => bail!(e::CcpiCompression),
	}

	g.slice(&pal);
	let (cw, ch, ccpi) = encode_ccpi(itp.width as usize, itp.height as usize, pixels);
	g.slice(&ccpi);

	let mut f = Writer::new();
	f.u32((g.len() + 16) as u32);
	f.slice(b"CCPI");
	f.u16(7); // version
	f.u16(pal_size as u16);
	f.u8(cw.ilog2() as u8);
	f.u8(ch.ilog2() as u8);
	f.u16(itp.width as u16);
	f.u16(itp.height as u16);
	f.u16(flags);
	f.slice(&maybe_compress(itp.status.compression, &g.finish()?));
	Ok(f.finish()?)
}

fn encode_ccpi(w: usize, h: usize, pixels: &[u8]) -> (usize, usize, Vec<u8>) {
	// 16*32 pixels means 8*16 tiles, which is guaranteed to be less than
	let cw = 16;
	let ch = 32;
	let mut out = Vec::new();
	for y in (0..h).step_by(ch) {
		for x in (0..w).step_by(cw) {
			let cw = cw.min(w - x);
			let ch = ch.min(h - y);
			let mut chunk = Vec::new();
			for y in y..y + ch {
				for x in x..x + cw {
					chunk.push(pixels[y * w + x]);
				}
			}
			permute::swizzle(&mut chunk, ch, cw, 2, 2);
			out.extend(encode_ccpi_chunk(&chunk));
		}
	}
	(cw, ch, out)
}

fn encode_ccpi_chunk(chunk: &[u8]) -> Vec<u8> {
	let mut v = Vec::new();
	let n = chunk.len() / 4;
	assert!(n < 255); // intentionally not <= since 0xFF means RLE
	v.push(n as u8);
	v.extend(chunk);
	v.extend(0..n as u8);
	v
}

fn a_fast_mode2(data: &[u8], width: u32, height: u32) -> Result<Vec<u8>, Error> {
	fn nibbles(f: &mut Writer, data: impl IntoIterator<Item = u8>) {
		let mut iter = data.into_iter();
		while let (Some(a), Some(b)) = (iter.next(), iter.next()) {
			f.u8(a << 4 | b)
		}
	}

	let data = do_swizzle(data.to_vec(), width as usize, height as usize, PFT::Pfp_1);

	let mut colors = Vec::new();
	let mut out = Vec::new();
	for chunk in data.array_chunks() {
		let mut chunk_colors = Vec::new();
		if chunk != &[0; 8 * 16] {
			for &a in chunk {
				out.push(
					chunk_colors
						.iter()
						.position(|i| i == &a)
						.unwrap_or_else(|| {
							chunk_colors.push(a);
							chunk_colors.len() - 1
						}) as u8,
				);
			}
		}
		if chunk_colors.len() == 1 {
			chunk_colors.push(0);
		}
		if chunk_colors.len() > 16 {
			bail!(e::AFastMode2Colors)
		}
		colors.push(chunk_colors);
	}

	let mut f = Writer::new();
	nibbles(
		&mut f,
		colors.iter().map(|a| a.len().saturating_sub(1) as u8),
	);
	for c in &colors {
		f.slice(c);
	}
	f.u8(0);
	nibbles(&mut f, out);
	Ok(f.finish()?)
}

fn maybe_compress(compression: CT, data: &[u8]) -> Vec<u8> {
	if compression == CT::None {
		data.to_owned()
	} else {
		// TODO
		falcompress::bzip::compress_ed7_to_vec(data, Default::default())
	}
}
