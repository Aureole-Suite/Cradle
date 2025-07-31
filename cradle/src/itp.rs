use crate::raster::Raster;
use gospel::read::Reader;
use num_enum::TryFromPrimitive;
use std::ffi::CString;

mod read;
mod write;

pub use read::Error as ReadError;
pub use write::Error as WriteError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Itp {
	pub status: ItpStatus,
	pub data: ImageData,
}

impl Itp {
	pub fn new(itp_revision: IR, data: ImageData) -> Itp {
		Itp {
			status: ItpStatus::default_for(itp_revision, &data),
			data,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageData {
	Indexed(Palette, Vec<Raster<u8>>),
	Argb16(Argb16Mode, Vec<Raster<u16>>),
	Argb32(Vec<Raster<u32>>),
	Bc1(Vec<Raster<u64>>),
	Bc2(Vec<Raster<u128>>),
	Bc3(Vec<Raster<u128>>),
	Bc7(Vec<Raster<u128>>),
}

impl ImageData {
	pub fn width(&self) -> usize {
		match self {
			ImageData::Indexed(_, d) => d[0].width(),
			ImageData::Argb16(_, d) => d[0].width(),
			ImageData::Argb32(d) => d[0].width(),
			ImageData::Bc1(d) => d[0].width() * 4,
			ImageData::Bc2(d) => d[0].width() * 4,
			ImageData::Bc3(d) => d[0].width() * 4,
			ImageData::Bc7(d) => d[0].width() * 4,
		}
	}

	pub fn height(&self) -> usize {
		match self {
			ImageData::Indexed(_, d) => d[0].height(),
			ImageData::Argb16(_, d) => d[0].height(),
			ImageData::Argb32(d) => d[0].height(),
			ImageData::Bc1(d) => d[0].height() * 4,
			ImageData::Bc2(d) => d[0].height() * 4,
			ImageData::Bc3(d) => d[0].height() * 4,
			ImageData::Bc7(d) => d[0].height() * 4,
		}
	}

	pub fn mipmaps(&self) -> usize {
		match self {
			ImageData::Indexed(_, d) => d.len(),
			ImageData::Argb16(_, d) => d.len(),
			ImageData::Argb32(d) => d.len(),
			ImageData::Bc1(d) => d.len(),
			ImageData::Bc2(d) => d.len(),
			ImageData::Bc3(d) => d.len(),
			ImageData::Bc7(d) => d.len(),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Argb16Mode {
	Mode1,
	Mode2,
	Mode3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

impl ItpStatus {
	pub fn default_for(itp_revision: ItpRevision, data: &ImageData) -> ItpStatus {
		let (base_format, pixel_bit_format) = match &data {
			ImageData::Indexed(_, _) => (BFT::Indexed1, PBFT::Indexed), // Indexed2/3 not supported
			ImageData::Argb16(A16::Mode1, _) => (BFT::Argb16, PBFT::Argb16_1),
			ImageData::Argb16(A16::Mode2, _) => (BFT::Argb16, PBFT::Argb16_2),
			ImageData::Argb16(A16::Mode3, _) => (BFT::Argb16, PBFT::Argb16_3),
			ImageData::Argb32(_) => (BFT::Argb32, PBFT::Argb32),
			ImageData::Bc1(_) => (BFT::Bc1, PBFT::Compressed),
			ImageData::Bc2(_) => (BFT::Bc2, PBFT::Compressed),
			ImageData::Bc3(_) => (BFT::Bc3, PBFT::Compressed),
			ImageData::Bc7(_) => (BFT::Bc7, PBFT::Compressed),
		};
		ItpStatus {
			itp_revision,
			base_format,
			compression: CT::None,
			pixel_format: PFT::Linear,
			pixel_bit_format,
			multi_plane: MPT::None,
			mipmap: if data.mipmaps() > 1 {
				MT::Mipmap_1
			} else {
				MT::None
			},
			use_alpha: None,
		}
	}
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

#[allow(non_camel_case_types)]
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

#[allow(non_camel_case_types)]
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

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, TryFromPrimitive)]
#[repr(u16)]
pub enum MipmapType {
	#[default]
	None = 0,
	Mipmap_1 = 1,
	Mipmap_2 = 2,
}

pub mod abbr {
	pub use super::Argb16Mode as A16;
	pub use super::BaseFormatType as BFT;
	pub use super::CompressionType as CT;
	pub use super::ItpRevision as IR;
	pub use super::MipmapType as MT;
	pub use super::MultiPlaneType as MPT;
	pub use super::PixelBitFormatType as PBFT;
	pub use super::PixelFormatType as PFT;
}

use abbr::*;

pub fn read(f: &[u8]) -> Result<Itp, read::Error> {
	read::read(&mut Reader::new(f))
}

pub fn read_size(f: &[u8]) -> Result<(usize, usize), read::Error> {
	read::read_size(&mut Reader::new(f))
}

pub fn write(itp: &Itp) -> Result<Vec<u8>, write::Error> {
	write::write(itp)
}

fn show_fourcc(fourcc: [u8; 4]) -> String {
	fourcc
		.iter()
		.flat_map(|a| std::ascii::escape_default(*a))
		.map(char::from)
		.collect()
}

#[cfg(test)]
#[filetest::filetest("../../samples/itp/*.itp")]
fn test_parse_all(bytes: &[u8]) -> Result<(), anyhow::Error> {
	let itp = read(bytes)?;
	let bytes2 = write(&itp)?;
	let itp2 = read(&bytes2)?;
	assert_eq!(itp, itp2);
	Ok(())
}
