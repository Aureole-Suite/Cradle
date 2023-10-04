#![allow(non_camel_case_types)]

use std::ffi::CString;
use num_enum::TryFromPrimitive;
use gospel::read::Reader;

mod read;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("{source}")]
	Read { #[from] source: gospel::read::Error, backtrace: std::backtrace::Backtrace },

	#[error("{source}")]
	Compression { #[from] source: falcompress::Error, backtrace: std::backtrace::Backtrace },

	#[error("this is not an itp file")]
	NotItp,

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

	#[error("wrong number of mipmaps: header says {expected}, but there are {value}")]
	WrongMips { expected: usize, value: usize },

	#[error("unexpected data after end")]
	RemainingData,

	#[error("ccpi only supports versions 6 and 7, got {0}")]
	CcpiVersion(u16),

	#[error("missing IHDR chunk")]
	NoHeader,

	#[error("base and pixel format mismatch: {bft:?} cannot use {pbft:?}")]
	PixelFormat { bft: BaseFormatType, pbft: PixelBitFormatType },

	#[error("got a palette on a non-indexed format")]
	PalettePresent,

	#[error("no palette is present for indexed format")]
	PaletteMissing,

	#[error("TODO: {0}")]
	TODO(String)
}

#[derive(Debug, Clone)]
pub struct Itp {
	pub status: ItpStatus,
	pub width: u32,
	pub height: u32,
	pub data: ImageData,
}

#[derive(Clone)]
pub enum ImageData {
	Indexed(Palette, Vec<u8>),
	Argb16_1(Vec<u16>),
	Argb16_2(Vec<u16>),
	Argb16_3(Vec<u16>),
	Argb32(Vec<u32>),
	Bc1(Vec<u64>),
	Bc2(Vec<u128>),
	Bc3(Vec<u128>),
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
	Pfp_2 = 2, // aka Tile
	Pfp_3 = 3, // aka Swizzle
	Pfp_4 = 4, // aka Tile or PS4Tile
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

pub fn read(f: &[u8]) -> Result<Itp, Error> {
	let f = &mut Reader::new(f);
	let itp = read::read(f)?;
	Ok(itp)
}

fn show_fourcc(fourcc: [u8; 4]) -> String {
	fourcc.iter()
		.flat_map(|a| std::ascii::escape_default(*a))
		.map(char::from)
		.collect()
}

#[cfg(test)]
#[filetest::filetest("../../samples/itp/*")]
fn test_parse_all(bytes: &[u8]) -> Result<(), anyhow::Error> {
	read(bytes)?;
	Ok(())
}
