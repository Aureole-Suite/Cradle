#![warn(clippy::todo)]

use gospel::write::{Writer, Le as _, Label};

use crate::permute;

use super::{Itp, ItpStatus, ImageData, Palette, Error, ItpError, abbr::*};

macro_rules! bail {
	($e:expr) => { { use ItpError::*; Err($e)?; unreachable!() } }
}

pub fn write(itp: &Itp) -> Result<Writer, Error> {
	let Itp { ref status, width, height, ref data } = *itp;

	let Some(head) = (match status.itp_revision {
		IR::V1 => status_to_flags(status).and_then(flags_to_gen1),
		IR::V2 => status_to_flags(status),
		IR::V3 => return write_revision_3(itp),
	}) else {
		bail!(Unrepresentable);
	};

	let mut f = Writer::new();
	f.u32(head);

	if status.base_format == BFT::Indexed3 {
		bail!(Todo("write_ccpi".into()))
	}

	f.u32(width);
	f.u32(height);

	if let ImageData::Indexed(pal, _) = data {
		let fixed_size = matches!(head, 1000 | 1002);
		let (is_external, pal_size, pal) = write_ipal(status, pal, fixed_size)?;
		if is_external {
			bail!(ExternalPalette)
		}
		if !fixed_size {
			f.u32(pal_size as u32);
		};
		f.append(pal);
	}

	for (width, height, range) in super::mipmaps(width, height, data.pixel_count()) {
		f.append(write_idat(status, width, height, data, range)?);
	}

	Ok(f)
}

fn write_revision_3(itp: &Itp) -> Result<Writer, Error> {
	fn chunk(f: &mut Writer, fourcc: &[u8; 4], body: Writer) {
		f.slice(fourcc);
		f.u32(body.len() as u32);
		f.append(body)
	}

	let Itp { ref status, width, height, ref data } = *itp;

	let end = Label::new();
	let start = Label::new();

	let mut f = Writer::new();
	f.label(start);
	f.slice(b"ITP\xFF");

	chunk(&mut f, b"IHDR", {
		let mut f = Writer::new();
		f.u32(32);
		f.u32(width);
		f.u32(height);
		f.delay(move |l| Ok(u32::to_le_bytes((l.label(end)? - l.label(start)?) as u32)));
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
			f.append(pal);
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

	for (n, (width, height, range)) in super::mipmaps(width, height, data.pixel_count()).enumerate() {
		chunk(&mut f, b"IDAT", {
			let mut f = Writer::new();
			f.u32(8);
			f.u16(0);
			f.u16(n as u16);
			f.append(write_idat(status, width, height, data, range)?);
			f
		});
	}

	chunk(&mut f, b"IEND", Writer::new());

	f.label(end);
	Ok(f)
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
		_ => return None
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

fn write_ipal(status: &ItpStatus, pal: &Palette, fixed_size: bool) -> Result<(bool, usize, Writer), Error> {
	match pal {
		Palette::Embedded(pal) => {
			let mut colors = pal.to_owned();
			if status.base_format == BFT::Indexed2 {
				for i in 1..colors.len() {
					colors[i] = colors[i].wrapping_sub(colors[i-1])
				}
			}

			if fixed_size {
				colors.resize(256, 0);
			}

			let mut f = Writer::new();
			for color in colors {
				f.u32(color);
			}

			Ok((false, pal.len(), maybe_compress(status.compression, &f.finish()?)?))
		},
		Palette::External(_) => bail!(Todo(String::from("external IPAL"))),
	}
}

fn write_idat(status: &ItpStatus, width: u32, height: u32, data: &ImageData, range: std::ops::Range<usize>) -> Result<Writer, Error> {
	match data {
		ImageData::Indexed(_, data) => match status.base_format {
			BFT::Indexed1 => write_idat_simple(&data[range], status, width, height, u8::to_le_bytes),
			BFT::Indexed2 => bail!(Todo("can not currently write AFastMode2".to_owned())),
			BFT::Indexed3 => bail!(Todo("CCPI is not supported for revision 3".to_owned())),
			_ => unreachable!()
		},
		ImageData::Argb16(_, data) => write_idat_simple(&data[range], status, width, height, u16::to_le_bytes),
		ImageData::Argb32(data) => write_idat_simple(&data[range], status, width, height, u32::to_le_bytes),
		ImageData::Bc1(data) => write_idat_simple(&data[range], status, width / 4, height / 4, u64::to_le_bytes),
		ImageData::Bc2(data) => write_idat_simple(&data[range], status, width / 4, height / 4, u128::to_le_bytes),
		ImageData::Bc3(data) => write_idat_simple(&data[range], status, width / 4, height / 4, u128::to_le_bytes),
		ImageData::Bc7(data) => write_idat_simple(&data[range], status, width / 4, height / 4, u128::to_le_bytes),
	}
}

fn write_idat_simple<T: Clone, const N: usize>(
	data: &[T],
	status: &ItpStatus,
	width: u32,
	height: u32,
	to_le_bytes: fn(T) -> [u8; N],
) -> Result<Writer, Error> {
	let mut data = data.to_vec();
	do_swizzle(&mut data, width as usize, height as usize, status.pixel_format);
	let data = data.into_iter().flat_map(to_le_bytes).collect::<Vec<u8>>();
	maybe_compress(status.compression, &data)
}

fn do_swizzle<T>(data: &mut [T], width: usize, height: usize, pixel_format: PFT) {
	match pixel_format {
		PFT::Linear => {},
		PFT::Pfp_1 => permute::swizzle(data, height, width, 8, 16),
		PFT::Pfp_2 => permute::swizzle(data, height, width, 32, 32),
		PFT::Pfp_3 => permute::morton(data, height, width),
		PFT::Pfp_4 => {
			permute::swizzle(data, height, width, 8, 1);
			permute::morton(data, width*height/8, 8);
		}
	}
}

fn maybe_compress(compression: CT, data: &[u8]) -> Result<Writer, Error> {
	let mut f = Writer::new();
	if compression == CT::None {
		f.slice(data)
	} else {
		// TODO
		falcompress::bzip::compress_ed7(&mut f, data, Default::default())
	}
	Ok(f)
}
