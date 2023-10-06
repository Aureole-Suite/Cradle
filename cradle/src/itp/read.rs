use falcompress::freadp::freadp;
use gospel::read::{Reader, Le as _};
use num_enum::{TryFromPrimitive, TryFromPrimitiveError};

use crate::permute;

use super::{Itp, ItpStatus, ImageData, Palette, Error, ItpError, abbr::*};

macro_rules! bail {
	($e:expr) => { { use ItpError::*; Err($e)?; unreachable!() } }
}

pub fn read(f: &mut Reader) -> Result<Itp, Error> {
	const ITP: u32 = u32::from_le_bytes(*b"ITP\xFF");
	const PNG: u32 = u32::from_le_bytes(*b"\x89PNG");
	const DDS: u32 = u32::from_le_bytes(*b"DDS ");

	let head = f.u32()?;
	let flags = match head {
		PNG | DDS => return Err(Error::NotItp),
		ITP => {
			f.seek(f.pos() - 4)?;
			return read_revision_3(f);
		}
		999  => 0x108802, // Argb16_2, None, Linear
		1000 => 0x108801, // Indexed1, None, Linear
		1001 => 0x110802, // Argb16_2, Bz_1, Linear
		1002 => 0x110801, // Indexed1, Bz_1, Linear
		1003 => 0x110402, // Argb16_2, Bz_1, Pfp_1
		1004 => 0x110401, // Indexed1, Bz_1, Pfp_1
		1005 => 0x210401, // Indexed2, Bz_1, Pfp_1
		1006 => 0x400401, // Indexed3, Ccpi, Pfp_1
		x if x & 0x40000000 != 0 => x,
		_ => return Err(Error::NotItp),
	};
	let status = status_from_flags(flags)?;

	if status.base_format == BFT::Indexed3 {
		return read_ccpi(f, status);
	}

	let mut data = make_data(&status)?;

	// Formats indexed1 and 2 seem to have a check for width == 0 here.
	// Seems to be something with palette, but no idea what.
	let width = f.u32()?;
	let height = f.u32()?;

	if let ImageData::Indexed(pal, _) = &mut data {
		let pal_size = if matches!(head, 1000 | 1002) { 256 } else { f.u32()? as usize };
		*pal = read_ipal(f, &status, false, pal_size)?;
	}

	read_idat(f, &status, &mut data, width, height)?;

	Ok(Itp { status, width, height, data })
}

fn read_revision_3(f: &mut Reader) -> Result<Itp, Error> {
	let start = f.pos();
	f.check(b"ITP\xFF")?;
	let mut width = 0;
	let mut height = 0;
	let mut file_size = 0;
	let mut n_mip = 0;
	let mut current_mip = 0;
	let mut status = ItpStatus::default();
	let mut pal = None;
	let mut data = None;

	loop {
		let fourcc = f.array::<4>()?;
		let _size = f.u32()? as usize;
		// Size is incorrect on both IPAL-having files I have
		match &fourcc {
			b"IHDR" => {
				f.check_u32(32)?;
				width = f.u32()?;
				height = f.u32()?;
				file_size = f.u32()? as usize;
				status.itp_revision = f.enum16("IHDR.itp_revision")?;
				status.base_format = f.enum16("IHDR.base_format")?;
				status.pixel_format = f.enum16("IHDR.pixel_format")?;
				status.pixel_bit_format = f.enum16("IHDR.pixel_bit_format")?;
				status.compression = f.enum16("IHDR.compression")?;
				status.multi_plane = f.enum16("IHDR.multi_plane")?;
				f.check_u32(0)?;
				data = Some(make_data(&status)?)
			}

			b"IMIP" => {
				f.check_u32(12)?;
				status.mipmap = f.enum16("IMIP.mipmap")?;
				n_mip = f.u16()? as usize;
				f.check_u32(0)?;
			}

			b"IHAS" => {
				f.check_u32(16)?;
				f.check_u32(0)?;
				f.array::<8>()?;
			}

			b"IPAL" => {
				f.check_u32(8)?;
				let is_external = f.bool16("IPAL.is_external")?;
				let pal_size = f.u16()? as usize;
				pal = Some(read_ipal(f, &status, is_external, pal_size)?);
			}

			b"IALP" => {
				f.check_u32(8)?;
				status.use_alpha = Some(f.bool16("IALP.use_alpha")?);
				f.check_u16(0)?;
			}

			b"IDAT" => {
				f.check_u32(8)?;
				f.check_u16(0)?;
				f.check_u16(current_mip as u16)?;
				let Some(data) = &mut data else {
					bail!(NoHeader)
				};
				read_idat(f, &status, data, width >> current_mip, height >> current_mip)?;
				current_mip += 1;
			}

			b"IEXT" => bail!(Todo("IEXT chunk".into())),

			b"IEND" => break,
			_ => bail!(BadChunk { fourcc })
		}
	}

	let Some(mut data) = data else {
		bail!(NoHeader)
	};

	if let Some(palette) = pal {
		let ImageData::Indexed(pal, _) = &mut data else {
			bail!(PalettePresent);
		};
		*pal = palette;
	} else if let ImageData::Indexed(..) = data {
		bail!(PaletteMissing);
	}

	ensure_size(f.pos() - start, file_size)?;

	if n_mip + 1 != current_mip {
		bail!(WrongMips { expected: n_mip + 1, value: current_mip });
	}

	Ok(Itp { status, width, height, data })
}

fn status_from_flags(f: u32) -> Result<ItpStatus, Error> {
	macro_rules! bits {
		($($bit:expr => $v:expr,)* _ => $def:expr) => {
			$(if f & (1<<$bit) != 0 { $v } else)* { $def }
		}
	}

	let itp_revision = bits! {
		30 => IR::V2,
		_ => IR::V1
	};

	let (base_format, pixel_bit_format) = bits! {
		0 => bits! {
			20 => (BFT::Indexed1, PBFT::Indexed),
			21 => (BFT::Indexed2, PBFT::Indexed),
			22 => (BFT::Indexed3, PBFT::Indexed),
			_ => bail!(MissingFlag("indexed type"))
		},
		3 => (BFT::Argb16, PBFT::Argb16_1),
		1 => (BFT::Argb16, PBFT::Argb16_2),
		2 => (BFT::Argb16, PBFT::Argb16_3),
		4 => (BFT::Argb32, PBFT::Argb32),
		24 => (BFT::Bc1, PBFT::Compressed),
		25 => (BFT::Bc2, PBFT::Compressed),
		26 => (BFT::Bc3, PBFT::Compressed),
		_ => bail!(MissingFlag("base format type"))
	};

	let compression = bits! {
		15 => CT::None,
		16 => bits! {
			17 => CT::Bz_2,
			_ => CT::Bz_1
		},
		_ => CT::None // ccpi
	};

	let pixel_format = bits! {
		10 => PFT::Pfp_1,
		11 => PFT::Linear,
		12 => PFT::Pfp_2,
		13 => PFT::Pfp_3,
		14 => PFT::Pfp_4,
		_ => bail!(MissingFlag("pixel format"))
	};

	let multi_plane = MPT::None;

	let mipmap = MT::None;

	let use_alpha = bits! {
		28 => Some(true),
		29 => Some(false),
		_ => None
	};

	let unused: u32 = [5, 6, 7, 8, 9, 18, 19, 23, 27, 31].iter().map(|a| 1 << *a).sum();
	if f & unused != 0 {
		bail!(ExtraFlags(f & unused))
	}

	Ok(ItpStatus {
		itp_revision,
		base_format,
		compression,
		pixel_format,
		pixel_bit_format,
		multi_plane,
		mipmap,
		use_alpha,
	})
}

fn read_ipal(f: &mut Reader, status: &ItpStatus, is_external: bool, size: usize) -> Result<Palette, Error> {
	if is_external {
		bail!(Todo(String::from("External IPAL")));
	} else {
		let data = read_maybe_compressed(f, status.compression, size * 4)?;

		let g = &mut Reader::new(&data);
		let mut colors = Vec::with_capacity(size);
		for _ in 0..size {
			colors.push(g.u32()?);
		}

		if status.base_format == BFT::Indexed2 {
			for i in 1..size {
				colors[i] = colors[i].wrapping_add(colors[i-1])
			}
		}
		Ok(Palette::Embedded(colors))
	}
}

fn read_idat(f: &mut Reader, status: &ItpStatus, data: &mut ImageData, width: u32, height: u32) -> Result<(), Error> {
	match data {
		ImageData::Indexed(_, data) => match status.base_format {
			BFT::Indexed1 => data.extend(read_idat_simple(f, status, width, height, u8::from_le_bytes)?),
			BFT::Indexed2 => data.extend({
				let size = f.u32()? as usize;
				let data = read_maybe_compressed(f, status.compression, size)?;
				let g = &mut Reader::new(&data);
				let data = a_fast_mode2(g, width, height)?;
				ensure_end(g)?;
				data
			}),
			BFT::Indexed3 => bail!(Todo("CCPI is not supported for revision 3".to_owned())),
			_ => unreachable!()
		},
		ImageData::Argb16(_, data) => data.extend(read_idat_simple(f, status, width, height, u16::from_le_bytes)?),
		ImageData::Argb32(data) => data.extend(read_idat_simple(f, status, width, height, u32::from_le_bytes)?),
		ImageData::Bc1(data) => data.extend(read_idat_simple(f, status, width / 4, height / 4, u64::from_le_bytes)?),
		ImageData::Bc2(data) => data.extend(read_idat_simple(f, status, width / 4, height / 4, u128::from_le_bytes)?),
		ImageData::Bc3(data) => data.extend(read_idat_simple(f, status, width / 4, height / 4, u128::from_le_bytes)?),
		ImageData::Bc7(data) => data.extend(read_idat_simple(f, status, width / 4, height / 4, u128::from_le_bytes)?),
	}
	Ok(())
}

fn read_idat_simple<T, const N: usize>(
	f: &mut Reader,
	status: &ItpStatus,
	width: u32,
	height: u32,
	from_le_bytes: fn([u8; N]) -> T,
) -> Result<Vec<T>, Error> {
	let data = read_maybe_compressed(f, status.compression, (width * height) as usize * N)?;
	let mut data = data.array_chunks().copied().map(from_le_bytes).collect::<Vec<_>>();
	do_unswizzle(&mut data, width as usize, height as usize, status.pixel_format);
	Ok(data)
}

fn do_unswizzle<T>(data: &mut [T], width: usize, height: usize, pixel_format: PFT) {
	match pixel_format {
		PFT::Linear => {},
		PFT::Pfp_1 => permute::unswizzle(data, height, width, 8, 16),
		PFT::Pfp_2 => permute::unswizzle(data, height, width, 32, 32),
		PFT::Pfp_3 => permute::unmorton(data, height, width),
		PFT::Pfp_4 => {
			permute::unmorton(data, width*height/8, 8);
			permute::unswizzle(data, height, width, 8, 1);
		}
	}
}

fn make_data(status: &ItpStatus) -> Result<ImageData, Error> {
	Ok(match (status.base_format, status.pixel_bit_format) {
		(BFT::Indexed1 | BFT::Indexed2 | BFT::Indexed3, PBFT::Indexed) =>
			ImageData::Indexed(Palette::Embedded(Vec::new()), Vec::new()),
		(BFT::Argb16, PBFT::Argb16_1) => ImageData::Argb16(A16::Mode1, Vec::new()),
		(BFT::Argb16, PBFT::Argb16_2) => ImageData::Argb16(A16::Mode2, Vec::new()),
		(BFT::Argb16, PBFT::Argb16_3) => ImageData::Argb16(A16::Mode3, Vec::new()),
		(BFT::Argb32, PBFT::Argb32) => ImageData::Argb32(Vec::new()),
		(BFT::Bc1, PBFT::Compressed) => ImageData::Bc1(Vec::new()),
		(BFT::Bc2, PBFT::Compressed) => ImageData::Bc2(Vec::new()),
		(BFT::Bc3, PBFT::Compressed) => ImageData::Bc3(Vec::new()),
		(BFT::Bc7, PBFT::Compressed) => ImageData::Bc7(Vec::new()),
		(bft, pbft) => bail!(PixelFormat { bft, pbft }),
	})
}

fn read_ccpi(f: &mut Reader, mut status: ItpStatus) -> Result<Itp, Error> {
	let data_size = f.u32()? as usize;
	f.check(b"CCPI")?;

	let version = f.u16()?; // ys8 only accepts 6 and 7, which are also the only ones I've seen
	let pal_size = f.u16()? as usize;
	let cw = 1 << f.u8()? as usize;
	let ch = 1 << f.u8()? as usize;
	let w = f.u16()? as usize;
	let h = f.u16()? as usize;
	let flags = f.u16()?;

	if !matches!(version, 6 | 7) {
		bail!(CcpiVersion(version));
	}

	status.compression = if flags & 0x8000 != 0 { CT::Bz_1 } else { CT::None };
	let data = read_maybe_compressed(f, status.compression, data_size - 16)?;
	let f = &mut Reader::new(&data);

	let pal = if flags & (1<<9) != 0 {
		// TODO ensure palette size is 0?
		Palette::External(f.cstr()?.to_owned()) // palette file name
	} else {
		let mut pal = Vec::with_capacity(pal_size);
		for _ in 0..pal_size {
			pal.push(f.u32()?);
		}
		Palette::Embedded(pal)
	};

	let mut pixels = vec![0; w*h];
	for y in (0..h).step_by(ch) {
		for x in (0..w).step_by(cw) {
			let cw = cw.min(w-x);
			let ch = ch.min(h-y);
			let mut chunk = read_ccpi_chunk(f, cw * ch)?;
			permute::unswizzle(&mut chunk, ch, cw, 2, 2);
			let mut it = chunk.into_iter();
			for y in y..y+ch {
				for x in x..x+cw {
					pixels[y * w + x] = it.next().unwrap();
				}
			}
		}
	}
	ensure_end(f)?;

	Ok(Itp {
		status,
		width: w as u32,
		height: h as u32,
		data: ImageData::Indexed(pal, pixels),
	})
}

fn read_ccpi_chunk(f: &mut Reader, len: usize) -> Result<Vec<u8>, Error> {
	let mut tiles = [[0;4]; 256];
	let n = f.u8()? as usize;
	#[allow(clippy::needless_range_loop)]
	for i in 0..n {
		tiles[i] = f.array::<4>()?;
	}
	for i in n..(n*2).min(256) {
		let [a,b,c,d] = tiles[i-n];
		tiles[i] = [b,a,d,c]; // x-flip
	}
	for i in n*2..(n*4).min(256) {
		let [a,b,c,d] = tiles[i-n*2];
		tiles[i] = [c,d,a,b]; // y-flip
	}

	let mut chunk = Vec::with_capacity(len);
	let mut last = 0;
	while chunk.len() < len {
		match f.u8()? {
			0xFF => {
				for _ in 0..f.u8()? {
					chunk.extend(tiles[last]);
				}
			}
			v => {
				last = v as usize;
				chunk.extend(tiles[last])
			}
		}
	}
	ensure_size(chunk.len(), len)?;
	Ok(chunk)
}

fn a_fast_mode2(f: &mut Reader, width: u32, height: u32) -> Result<Vec<u8>, Error> {
	fn nibbles(f: &mut Reader, out: &mut [u8]) -> Result<(), Error> {
		for i in 0..out.len()/2 {
			let x = f.u8()?;
			out[2*i] = x >> 4;
			out[2*i+1] = x & 15;
		}
		Ok(())
	}

	let mut ncolors = vec![0; ((height/8)*(width/16)) as usize];
	nibbles(f, &mut ncolors)?;
	for a in &mut ncolors {
		if *a != 0 {
			*a += 1;
		}
	}

	let totalcolors = ncolors.iter().map(|a| *a as usize).sum::<usize>();
	let c = &mut Reader::new(f.slice(totalcolors)?);
	let mode = f.u8()?;

	let mut data = Vec::with_capacity((height*width) as usize);
	for ncolors in ncolors {
		let mut chunk = [0; 8*16];
		if ncolors != 0 {
			let colors = c.slice(ncolors as usize)?;
			match mode {
				0 => {
					nibbles(f, &mut chunk)?;
					chunk = chunk.map(|a| colors[a as usize]);
				}
				1 => bail!(Todo("obscure AFastMode2 subformat".into())),
				_ => {
					match f.u8()? {
						1 => {
							let mut toggle = false;
							#[allow(clippy::needless_range_loop)]
							for j in 0..16 {
								let mut pos = 0;
								loop {
									let m = f.u8()? as usize;
									if m == 0xFF { break; }
									if toggle {
										chunk[pos..pos+m+1].fill(colors[j]);
										pos += 2;
									}
									pos += m;
									toggle = !toggle;
								}
							}
						}
						n => bail!(Todo(format!("obscure AFastMode2 subformat {n}")))
					}
				}
			}
		}
		data.extend(chunk);
	}
	ensure_end(c)?;

	permute::unswizzle(&mut data, height as usize, width as usize, 8, 16);

	Ok(data)
}

fn read_maybe_compressed(f: &mut Reader, comp: CT, len: usize) -> Result<Vec<u8>, Error> {
	// Reader seems to make no difference between Bz_1 and C77. Guess writer does though?
	let data = match comp {
		CT::None => f.slice(len)?.to_vec(),
		CT::Bz_1 => freadp(f)?,
		CT::Bz_2 => freadp_multi(f, len)?,
		CT::C77 => freadp(f)?,
	};
	ensure_size(data.len(), len)?;
	Ok(data)
}

fn freadp_multi(f: &mut Reader, len: usize) -> Result<Vec<u8>, Error> {
	let mut out = Vec::new();
	while out.len() < len {
		out.extend(freadp(f)?)
	}
	Ok(out)
}

fn ensure_end(f: &Reader) -> Result<(), Error> {
	if f.remaining().is_empty() {
		Ok(())
	} else {
		bail!(RemainingData)
	}
}

fn ensure_size(value: usize, expected: usize) -> Result<(), Error> {
	if value == expected {
		Ok(())
	} else {
		bail!(WrongSize { value, expected })
	}
}

#[extend::ext(name = ReaderExt2)]
pub impl Reader<'_> {
	fn enum16<T: TryFromPrimitive<Primitive=u16,Error=TryFromPrimitiveError<T>>>(&mut self, field: &'static str) -> Result<T, Error> {
		T::try_from_primitive(self.u16()?)
			.map_err(|e| ItpError::Invalid { field, value: e.number as u32 }.into())
	}

	fn bool16(&mut self, field: &'static str) -> Result<bool, Error> {
		match self.u16()? {
			0 => Ok(false),
			1 => Ok(true),
			v => Err(ItpError::Invalid { field, value: v as u32 }.into())
		}
	}
}
