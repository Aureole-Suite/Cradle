#![allow(non_camel_case_types)]

use std::ffi::CString;
use num_enum::{TryFromPrimitive, TryFromPrimitiveError};
use gospel::read::{Reader, Le as _};
use falcompress::bzip;

use crate::util::swizzle_mut;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("{source}")]
	Read { #[from] source: gospel::read::Error, backtrace: std::backtrace::Backtrace },

	#[error("{source}")]
	Bzip { #[from] source: bzip::Error, backtrace: std::backtrace::Backtrace },

	#[error("{source}")]
	Itp {
		#[allow(private_interfaces)]
		#[from]
		source: ItpError,
		backtrace: std::backtrace::Backtrace,
	},
}

#[derive(Debug, thiserror::Error)]
enum ItpError {
	#[error("not an itp file")]
	NotItp,

	#[error("gen2 flags missing for {0}")]
	MissingFlag(&'static str),

	#[error("gen2 extra flags: {0:032b}")]
	ExtraFlags(u32),

	#[error("bad itp chunk '{}'", show_fourcc(*fourcc))]
	BadChunk { fourcc: [u8; 4] },

	#[error("invalid value for {field}: {value}")]
	Invalid { field: &'static str, value: u32 },

	#[error("unexpected size: expected {expected}, but got {value}")]
	WrongSize { expected: usize, value: usize },

	#[error("unexpected data after end")]
	RemainingData,

	#[error("ccpi only supports versions 6 and 7, got {0}")]
	CcpiVersion(u16),

	#[error("got a palette on a non-indexed format")]
	PalettePresent,

	#[error("no palette is present for indexed format")]
	PaletteMissing,

	#[error("TODO")]
	TODO
}

macro_rules! bail {
	($e:expr) => { { use ItpError::*; Err($e)?; unreachable!() } }
}

#[derive(Debug, Clone)]
pub struct Itp {
	pub status: ItpStatus,
	pub width: u32,
	pub height: u32,
	pub data: ImageData,
}

#[derive(Clone)] // XXX this Debug is no good
pub enum ImageData {
	Indexed(Palette, Vec<u8>),
	Argb16_1(Vec<u16>),
	Argb16_2(Vec<u16>),
	Argb16_3(Vec<u16>),
	Argb32(Vec<u32>),
	Bc1(Vec<u64>),
	Bc2(Vec<[u64; 2]>),
	Bc3(Vec<[u64; 2]>),
	Bc7(Vec<u128>),
}

impl std::fmt::Debug for ImageData {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Indexed(pal, data) => f.debug_tuple("Indexed").field(pal).field(&data.len()).finish(),
			Self::Argb16_1(data) => f.debug_tuple("Argb16_1").field(&data.len()).finish(),
			Self::Argb16_2(data) => f.debug_tuple("Argb16_2").field(&data.len()).finish(),
			Self::Argb16_3(data) => f.debug_tuple("Argb16_3").field(&data.len()).finish(),
			Self::Argb32(data) => f.debug_tuple("Argb32").field(&data.len()).finish(),
			Self::Bc1(data) => f.debug_tuple("Bc1").field(&data.len()).finish(),
			Self::Bc2(data) => f.debug_tuple("Bc2").field(&data.len()).finish(),
			Self::Bc3(data) => f.debug_tuple("Bc3").field(&data.len()).finish(),
			Self::Bc7(data) => f.debug_tuple("Bc7").field(&data.len()).finish(),
		}
	}
}

#[derive(Debug, Clone)]
pub enum Palette {
	Embedded(Vec<u32>),
	External(CString),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItpStatus {
	itp_revision: ItpRevision,
	base_format: BaseFormatType,
	compression: CompressionType,
	pixel_format: PixelFormatType,
	pixel_bit_format: PixelBitFormatType,
	multi_plane: MultiPlaneType,
	mipmap: MipmapType,
	use_alpha: bool,
}

impl ItpStatus {
	pub fn from_flags(f: u32) -> Result<ItpStatus, Error> {
		use ItpRevision as IR;
		use BaseFormatType as BFT;
		use PixelBitFormatType as PBFT;
		use CompressionType as CT;
		use PixelFormatType as PFT;

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
			12 => PFT::Tile_1,
			13 => if f & (7 << 24) != 0 { // For DXT formats
				PFT::Linear
			} else {
				PFT::Swizzle_1
			},
			14 => PFT::Ps4Tile,
			_ => bail!(MissingFlag("pixel format"))
		};

		let multi_plane = MultiPlaneType::None;

		let mipmap = MipmapType::None;

		let use_alpha = bits! {
			28 => true,
			29 => false,
			_ => true
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, TryFromPrimitive)]
#[repr(u16)]
enum ItpRevision {
	V1 = 1, // 999..=1006
	V2 = 2, // flag-based
	#[default]
	V3 = 3, // ITP\xFF
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, TryFromPrimitive)]
#[repr(u16)]
enum BaseFormatType {
	Indexed1 = 0, // 256 color
	Indexed2 = 1,
	Indexed3 = 2,
	// 3 is invalid
	Argb16 = 4, // 16bit color
	#[default]
	Argb32 = 5, // 32bit color
	Bc1 = 6,
	Bc2 = 7,
	Bc3 = 8,
	#[deprecated]
	BcAuto_1_3 = 9,
	Bc7 = 10,
}

impl BaseFormatType {
	fn bpp(self) -> usize {
		match self {
			BaseFormatType::Indexed1 => 8,
			BaseFormatType::Indexed2 => 8,
			BaseFormatType::Indexed3 => 8,
			BaseFormatType::Argb16 => 16,
			BaseFormatType::Argb32 => 32,
			BaseFormatType::Bc1 => 4,
			BaseFormatType::Bc2 => 8,
			BaseFormatType::Bc3 => 8,
			BaseFormatType::BcAuto_1_3 => 0,
			BaseFormatType::Bc7 => 8,
		}
	}

	fn is_indexed(&self) -> bool {
		matches!(self, Self::Indexed1 | Self::Indexed2 | Self::Indexed3)
	}

	fn is_bc(&self) -> bool {
		matches!(self, Self::Bc1 | Self::Bc2 | Self::Bc3 | Self::BcAuto_1_3 | Self::Bc7)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, TryFromPrimitive)]
#[repr(u16)]
enum PixelBitFormatType {
	Indexed = 0,
	Argb16_1 = 1,
	Argb16_2 = 2,
	Argb16_3 = 3,
	#[deprecated]
	Argb16_auto = 4,
	#[default]
	Argb32 = 5,
	Compressed = 6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, TryFromPrimitive)]
#[repr(u16)]
enum PixelFormatType {
	#[default]
	Linear = 0,
	Pfp_1 = 1,
	Tile_1 = 2,
	Swizzle_1 = 3,
	Ps4Tile = 4, // aka Tile
	Morton = 5, // aka Swizzle
	Pfp_6 = 6,
	Pfp_7 = 7,
	Pfp_8 = 8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, TryFromPrimitive)]
#[repr(u16)]
enum CompressionType {
	#[default]
	None = 0,
	Bz_1 = 1,
	Bz_2 = 2,
	C77 = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, TryFromPrimitive)]
#[repr(u16)]
enum MultiPlaneType {
	#[default]
	None = 0,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, TryFromPrimitive)]
#[repr(u16)]
enum MipmapType {
	#[default]
	None = 0,
	Mipmap_1 = 1,
	Mipmap_2 = 2,
}

pub fn read(f: &mut Reader) -> Result<Itp, Error> {
	use BaseFormatType as BFT;

	let head = f.u32()?;
	if head == u32::from_le_bytes(*b"ITP\xFF") {
		f.seek(f.pos() - 4)?;
		return read_revision_3(f);
	}

	let flags = match head {
		999  => 0x108802, // Argb16_2, None, Linear
		1000 => 0x108801, // Indexed1, None, Linear
		1001 => 0x110802, // Argb16_2, Bz_1, Linear
		1002 => 0x110801, // Indexed1, Bz_1, Linear
		1003 => 0x110402, // Argb16_2, Bz_1, Pfp_1
		1004 => 0x110401, // Indexed1, Bz_1, Pfp_1
		1005 => 0x210401, // Indexed2, Bz_1, Pfp_1
		1006 => 0x400401, // Indexed3, Ccpi, Pfp_1
		x if x & 0x40000000 != 0 => x,
		_ => bail!(NotItp),
	};
	let status = ItpStatus::from_flags(flags)?;

	if status.base_format == BFT::Indexed3 {
		return read_ccpi(f, status);
	}

	// Formats indexed1 and 2 seem to have a check for width == 0 here.
	// Seems to be something with palette, but no idea what.
	let width = f.u32()? as usize;
	let height = f.u32()? as usize;

	let pal = if status.base_format.is_indexed() {
		let pal_size = if matches!(head, 1000 | 1002) { 256 } else { f.u32()? as usize };
		Some(read_ipal(f, &status, false, pal_size)?)
	} else {
		None
	};

	let data = read_idat(f, &status, width, height, pal.as_ref())?;

	Ok(Itp {
		status,
		width: width as u32,
		height: width as u32,
		data,
	})
}

fn read_ccpi(f: &mut Reader, mut status: ItpStatus) -> Result<Itp, Error> {
	use CompressionType as CT;

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
			swizzle_mut(&mut chunk, ch/2, 2, cw/2, 2);
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

fn read_revision_3(f: &mut Reader) -> Result<Itp, Error> {
	let start = f.pos();
	f.check(b"ITP\xFF")?;
	let mut width = 0;
	let mut height = 0;
	let mut file_size = 0;
	let mut has_mip = false;
	let mut n_mip = 0;
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
				width = f.u32()? as usize;
				height = f.u32()? as usize;
				file_size = f.u32()? as usize;
				status.itp_revision = f.enum16("IHDR.itp_revision")?;
				status.base_format = f.enum16("IHDR.base_format")?;
				status.pixel_format = f.enum16("IHDR.pixel_format")?;
				status.pixel_bit_format = f.enum16("IHDR.pixel_bit_format")?;
				status.compression = f.enum16("IHDR.compression")?;
				status.multi_plane = f.enum16("IHDR.multi_plane")?;
				f.check_u32(0)?;
			}

			b"IALP" => {
				f.check_u32(8)?;
				status.use_alpha = f.bool16("IALP.use_alpha")?;
				f.check_u16(0)?;
			}

			b"IMIP" => {
				f.check_u32(12)?;
				status.mipmap = f.enum16("IMIP.mipmap")?;
				n_mip = f.u16()?;
				f.check_u32(0)?;
			}

			b"IPAL" => {
				f.check_u32(8)?;
				let is_external = f.bool16("IPAL.is_external")?;
				let pal_size = f.u16()? as usize;
				pal = Some(read_ipal(f, &status, is_external, pal_size)?);
			}

			b"IDAT" => {
				f.check_u32(8)?;
				f.check_u16(0)?;
				let mip_nr = f.u16()?;
				data = Some(read_idat(f, &status, width, height, pal.as_ref())?);
			}

			b"IEXT" => unimplemented!(),

			b"IHAS" => {
				f.check_u32(16)?;
				f.check_u32(0)?;
				f.array::<8>()?;
			}

			b"IEND" => break,
			_ => bail!(BadChunk { fourcc })
		}
	}

	Ok(Itp {
		status,
		width: width as u32,
		height: height as u32,
		data: data.unwrap(), // XXX
	})
}

fn read_ipal(f: &mut Reader, status: &ItpStatus, is_external: bool, size: usize) -> Result<Palette, Error> {
	if is_external {
		bail!(TODO);
	} else {
		let data = read_maybe_compressed(f, status.compression, size * 4)?;

		let g = &mut Reader::new(&data);
		let mut colors = Vec::with_capacity(size);
		for _ in 0..size {
			colors.push(g.u32()?);
		}

		if status.base_format == BaseFormatType::Indexed2 {
			for i in 1..size {
				colors[i] = colors[i].wrapping_add(colors[i-1])
			}
		}
		Ok(Palette::Embedded(colors))
	}
}

fn read_idat(f: &mut Reader, status: &ItpStatus, width: usize, height: usize, palette: Option<&Palette>) -> Result<ImageData, Error> {
	use BaseFormatType as BFT;
	let bft = status.base_format;
	let len = width * height * bft.bpp() / 8;
	if palette.is_some() && !bft.is_indexed() {
		bail!(PalettePresent);
	}
	if palette.is_none() && bft.is_indexed() {
		bail!(PaletteMissing);
	}
	match bft {
		BFT::Indexed1 => {
			let mut data = read_maybe_compressed(f, status.compression, len)?;
			if status.pixel_format == PixelFormatType::Pfp_1 {
				swizzle_mut(&mut data, height/8, width/16, 8, 16);
			}
			Ok(ImageData::Indexed(palette.unwrap().clone(), data))
		}
		BFT::Indexed2 => {
			bail!(TODO)
			// let mut data = a_fast_mode2(f, width, height);
			// swizzle_mut(&mut data, height/8, width/16, 8, 16);
			// data
		}
		BFT::Indexed3 => {
			panic!("CCPI is not supported for revision 3")
		}
		BFT::Argb32 => {
			let data = read_maybe_compressed(f, status.compression, len)?;
			let data = data.array_chunks().copied().map(u32::from_le_bytes).collect();
			Ok(ImageData::Argb32(data))
		}
		_ => {
			bail!(TODO)
		}
	}
}

fn read_maybe_compressed(f: &mut Reader, comp: CompressionType, len: usize) -> Result<Vec<u8>, Error> {
	use CompressionType as CT;
	let data = match comp {
		CT::None => f.slice(len)?.to_vec(),
		CT::Bz_1 | CT::C77 => freadp(f, Some(len))?,
		CT::Bz_2 => freadp_multi(f, len)?,
	};
	ensure_size(data.len(), len)?;
	Ok(data)
}

fn freadp_multi(f: &mut Reader, len: usize) -> Result<Vec<u8>, Error> {
	let mut out = Vec::new();
	while out.len() < len {
		out.extend(freadp(f, None)?)
	}
	Ok(out)
}

fn freadp(f: &mut Reader, expected_len: Option<usize>) -> Result<Vec<u8>, Error> {
	if f.check_u32(0x80000001).is_ok() {
		let n_chunks = f.u32()? as usize;
		let total_csize = f.u32()? as usize;
		let buf_size = f.u32()? as usize;
		let total_usize = f.u32()? as usize;
		if let Some(len) = expected_len {
			ensure_size(total_usize, len)?;
		}
		let f = &mut Reader::new(f.slice(total_csize)?);

		let mut data = Vec::new();
		let mut max_csize = 0;
		for _ in 0..n_chunks {
			let start = f.pos();
			decompress_c77(f, &mut data)?;
			max_csize = max_csize.max(f.pos() - start);
		}
		ensure_size(max_csize, buf_size)?;
		ensure_size(data.len(), total_usize)?;
		ensure_end(f)?;
		Ok(data)
	} else {
		Ok(bzip::decompress_ed7(f)?)
	}
}

fn decompress_c77(f: &mut Reader, out: &mut Vec<u8>) -> Result<(), Error> {
	let csize = f.u32()? as usize;
	let usize = f.u32()? as usize;
	let data = f.slice(csize)?;

	let start = out.len();
	let mut f = Reader::new(data);
	let mode = f.u32()?;
	if mode == 0 {
		out.extend_from_slice(&data[4..]);
	} else {
		decompress_c77_inner(f, mode, out)?;
	}

	let written = out.len() - start;
	ensure_size(written, usize)?;
	Ok(())
}

fn decompress_c77_inner(mut f: Reader<'_>, mode: u32, out: &mut Vec<u8>) -> Result<(), Error> {
	let start = out.len();
	while !f.is_empty() {
		let x = f.u16()? as usize;
		let op = x & !(!0 << mode);
		let num = x >> mode;
		if op == 0 {
			out.extend(f.slice(num)?);
		} else {
			if num > out.len() - start {
				return Err(bzip::Error::BadRepeat { count: op, offset: num + 1, len: out.len() }.into())
			};
			for _ in 0..op {
				out.push(out[out.len() - num - 1])
			}
			out.push(f.u8()?);
		}
	}
	Ok(())
}


fn show_fourcc(fourcc: [u8; 4]) -> String {
	fourcc.iter()
		.flat_map(|a| std::ascii::escape_default(*a))
		.map(char::from)
		.collect()
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

#[cfg(test)]
#[filetest::filetest("../../samples/itp/*")]
fn test_parse_all(bytes: &[u8]) -> Result<(), anyhow::Error> {
	read(&mut Reader::new(bytes))?;
	Ok(())
}

#[cfg(test)]
#[test]
fn test_png() -> anyhow::Result<()> {
	let path = "../samples/itp/ys_celceta__f_00409.itp";
	let file = std::fs::File::open(path)?;
	let dat = unsafe { memmap2::Mmap::map(&file)? };
	let itp = read(&mut Reader::new(&dat))?;
	let Itp { status: _, width, height, data } = itp;
	let ImageData::Argb32(data) = data else { panic!() };
	write_png(std::fs::File::create("/tmp/a.png")?, width, height, &data)?;

	Ok(())
}

#[cfg(test)]
fn write_png(
	mut w: impl std::io::Write,
	width: u32,
	height: u32,
	data: &[u32],
) -> Result<(), anyhow::Error> {
	let mut png = png::Encoder::new(&mut w, width, height);
	png.set_color(png::ColorType::Rgba);
	png.set_depth(png::BitDepth::Eight);
	let mut w = png.write_header()?;
	let data: Vec<u8> =  data.iter()
		.flat_map(|argb| {
			let [b, g, r, a] = u32::to_le_bytes(*argb);
			[r, g, b, a]
		})
		.collect::<Vec<_>>();
	w.write_image_data(&data)?;
	w.finish()?;
	Ok(())
}

#[cfg(test)]
fn write_indexed_png(
	mut w: impl std::io::Write,
	width: u32,
	height: u32,
	palette: &[u32],
	data: &[u8],
) -> Result<(), anyhow::Error> {
	let mut png = png::Encoder::new(&mut w, width, height);
	let mut pal = Vec::with_capacity(3*palette.len());
	let mut alp = Vec::with_capacity(palette.len());
	for rgba in palette {
		let [r, g, b, a] = u32::to_le_bytes(*rgba);
		pal.push(r);
		pal.push(g);
		pal.push(b);
		alp.push(a);
	}
	png.set_color(png::ColorType::Indexed);
	png.set_depth(png::BitDepth::Eight);
	png.set_palette(pal);
	png.set_trns(alp);
	let mut w = png.write_header()?;
	w.write_image_data(data)?;
	w.finish()?;
	Ok(())
}
