#![allow(non_camel_case_types)]

use std::ffi::CString;
use num_enum::TryFromPrimitive;
use gospel::read::Reader;

mod read;
mod write;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("{source}")]
	Read { #[from] source: gospel::read::Error, backtrace: std::backtrace::Backtrace },

	#[error("{source}")]
	Write { #[from] source: gospel::write::Error, backtrace: std::backtrace::Backtrace },

	#[error("{source}")]
	Compression { #[from] source: falcompress::Error, backtrace: std::backtrace::Backtrace },

	#[error("this is not an itp file")]
	NotItp,

	#[error("{source}")]
	#[allow(private_interfaces)]
	Itp { #[from] source: ItpError, backtrace: std::backtrace::Backtrace },
}

#[derive(Debug, thiserror::Error)]
enum ItpError {
	#[error("gen2 flags missing for {0}")]
	MissingFlag(&'static str),

	#[error("gen2 extra flags: {0:08X}")]
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

	#[error("the specified revision cannot represent this file")]
	Unrepresentable,

	#[error("the specified format does not support external palettes")]
	ExternalPalette,

	#[error("TODO: {0}")]
	Todo(String)
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
	Argb16(Argb16Mode, Vec<u16>),
	Argb32(Vec<u32>),
	Bc1(Vec<u64>),
	Bc2(Vec<u128>),
	Bc3(Vec<u128>),
	Bc7(Vec<u128>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Argb16Mode {
	Mode1,
	Mode2,
	Mode3,
}

impl std::fmt::Debug for ImageData {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		match self {
			Self::Indexed(pal, data) => f.debug_tuple("Indexed").field(pal).field(&data.len()).finish(),
			Self::Argb16(mode, data) => f.debug_tuple("Argb16").field(mode).field(&data.len()).finish(),
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
	pub itp_revision: ItpRevision,
	pub base_format: BaseFormatType,
	pub compression: CompressionType,
	pub pixel_format: PixelFormatType,
	pub pixel_bit_format: PixelBitFormatType,
	pub multi_plane: MultiPlaneType,
	pub mipmap: MipmapType,
	pub use_alpha: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, TryFromPrimitive)]
#[repr(u16)]
pub enum ItpRevision {
	V1 = 1, // 999..=1006
	V2 = 2, // flag-based
	#[default]
	V3 = 3, // ITP\xFF
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, TryFromPrimitive)]
#[repr(u16)]
pub enum BaseFormatType {
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
	// BcAuto_1_3 = 9,
	Bc7 = 10,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, TryFromPrimitive)]
#[repr(u16)]
pub enum PixelBitFormatType {
	Indexed = 0,
	Argb16_1 = 1,
	Argb16_2 = 2,
	Argb16_3 = 3,
	// Argb16_auto = 4,
	#[default]
	Argb32 = 5,
	Compressed = 6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, TryFromPrimitive)]
#[repr(u16)]
pub enum PixelFormatType {
	#[default]
	Linear = 0,
	Pfp_1 = 1,
	Pfp_2 = 2, // aka Tile
	Pfp_3 = 3, // aka Swizzle
	Pfp_4 = 4, // aka Tile or PS4Tile
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, TryFromPrimitive)]
#[repr(u16)]
pub enum CompressionType {
	#[default]
	None = 0,
	Bz_1 = 1,
	Bz_2 = 2,
	C77 = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, TryFromPrimitive)]
#[repr(u16)]
pub enum MultiPlaneType {
	#[default]
	None = 0,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, TryFromPrimitive)]
#[repr(u16)]
pub enum MipmapType {
	#[default]
	None = 0,
	Mipmap_1 = 1,
	Mipmap_2 = 2,
}

pub mod abbr {
	pub use super::ItpRevision as IR;
	pub use super::BaseFormatType as BFT;
	pub use super::PixelBitFormatType as PBFT;
	pub use super::CompressionType as CT;
	pub use super::PixelFormatType as PFT;
	pub use super::MultiPlaneType as MPT;
	pub use super::MipmapType as MT;
	pub use super::Argb16Mode as A16;
}

pub fn read(f: &[u8]) -> Result<Itp, Error> {
	read::read(&mut Reader::new(f))
}

pub fn write(itp: &Itp) -> Result<Vec<u8>, Error> {
	Ok(write::write(itp)?.finish()?)
}

fn show_fourcc(fourcc: [u8; 4]) -> String {
	fourcc.iter()
		.flat_map(|a| std::ascii::escape_default(*a))
		.map(char::from)
		.collect()
}

impl ImageData {
	pub fn pixel_count(&self) -> usize {
		match self {
			ImageData::Indexed(_, d) => d.len(),
			ImageData::Argb16(_, d)  => d.len(),
			ImageData::Argb32(d)     => d.len(),
			ImageData::Bc1(d)        => d.len() * 16,
			ImageData::Bc2(d)        => d.len() * 16,
			ImageData::Bc3(d)        => d.len() * 16,
			ImageData::Bc7(d)        => d.len() * 16,
		}
	}
}

pub fn mipmaps(mut width: u32, mut height: u32, len: usize) -> impl Iterator<Item=(u32, u32, std::ops::Range<usize>)> {
	let mut pos = 0;
	std::iter::from_fn(move || {
		let size = (width*height) as usize;
		if size == 0 || pos + size > len {
			None
		} else {
			let val = (width, height, pos..pos+size);
			pos += size;
			width >>= 1;
			height >>= 1;
			Some(val)
		}
	})
}

#[cfg(test)]
#[filetest::filetest("../../samples/itp/*")]
fn test_parse_all(bytes: &[u8]) -> Result<(), anyhow::Error> {
	read(bytes)?;
	Ok(())
}
