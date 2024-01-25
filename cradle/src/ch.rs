use std::backtrace::Backtrace;

use crate::raster::Raster;
use crate::util::ensure;

#[derive(Debug, thiserror::Error)]
pub enum ReadError {
	#[error("{message}")]
	Whatever {
		message: String,
		backtrace: Backtrace,
	},
}

impl From<std::fmt::Arguments<'_>> for ReadError {
	fn from(message: std::fmt::Arguments<'_>) -> Self {
		Self::Whatever {
			message: message.to_string(),
			backtrace: Backtrace::capture(),
		}
	}
}

#[derive(Debug, thiserror::Error)]
pub enum WriteError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageData {
	Argb1555(Raster<u16>),
	Argb4444(Raster<u16>),
	Argb8888(Raster<u32>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
	Argb1555,
	Argb4444,
	Argb8888,
}

impl Mode {
	fn bytes_per(self) -> usize {
		match self {
			Mode::Argb1555 => 2,
			Mode::Argb4444 => 2,
			Mode::Argb8888 => 4,
		}
	}
}

pub fn read(mode: Mode, width: usize, ch: &[u8]) -> Result<ImageData, ReadError> {
	fn raster<T, const N: usize>(
		ch: &[u8],
		width: usize,
		from_le_bytes: fn([u8; N]) -> T,
	) -> Result<Raster<T>, ReadError> {
		let stride = width * N;
		ensure!(stride != 0, "invalid width");
		let height = ch.len() / stride;
		ensure!(height * stride == ch.len(), "invalid size");
		let data = ch.array_chunks().copied().map(from_le_bytes).collect();
		Ok(Raster::new_with(width, height, data))
	}
	Ok(match mode {
		Mode::Argb1555 => ImageData::Argb1555(raster(ch, width, u16::from_le_bytes)?),
		Mode::Argb4444 => ImageData::Argb4444(raster(ch, width, u16::from_le_bytes)?),
		Mode::Argb8888 => ImageData::Argb8888(raster(ch, width, u32::from_le_bytes)?),
	})
}

pub fn write(img: &ImageData) -> Result<Vec<u8>, WriteError> {
	fn raster<T: Copy, const N: usize>(
		img: &Raster<T>,
		to_le_bytes: fn(T) -> [u8; N],
	) -> Result<Vec<u8>, WriteError> {
		Ok(img
			.as_slice()
			.iter()
			.copied()
			.flat_map(to_le_bytes)
			.collect())
	}
	match img {
		ImageData::Argb1555(img) => raster(img, u16::to_le_bytes),
		ImageData::Argb4444(img) => raster(img, u16::to_le_bytes),
		ImageData::Argb8888(img) => raster(img, u32::to_le_bytes),
	}
}

macro_rules! guess {
	($($prefix:literal, $mode:ident, $w:literal, $h:literal;)*) => {
		pub fn guess_from_byte_size(name: &str, bytes: usize) -> Option<(Mode, usize, usize)> {
			$(if name.starts_with($prefix) && bytes == $w * $h * Mode::$mode.bytes_per() {
				return Some((Mode::$mode, $w, $h))
			})*
			None
		}
		pub fn guess_from_image_size(name: &str, w: usize, h: usize) -> Option<Mode> {
			$(if name.starts_with($prefix) && w == $w && h == $h {
				return Some(Mode::$mode)
			})*
			None
		}
	}
}

guess! {
	"c_ka",     Argb1555,  128,  128; // dialogue face
	"h_ka",     Argb1555,  256,  256;
	"c_stch",   Argb8888,  512,  512; // menu portrait
	"h_stch",   Argb8888, 1024, 1024;
	"cti",      Argb1555,  256,  256; // s-craft cut-in
	"bface",    Argb1555,  256,  256; // battle face
	"hface",    Argb1555,  512,  512;
	"m",        Argb4444, 1024, 1024; // minimap
	"ca",       Argb1555,  128,  128; // bestiary image
	"ca",       Argb1555,  256,  256;
	"c_note",   Argb1555,  768,  512; // notebook
	"h_note",   Argb1555, 1536, 1024;
	"c_epi",    Argb4444,  208,  176; // door thumbnails
	"h_epi",    Argb4444,  416,  352;
	"c_orb",    Argb1555,  512,  512; // character orbment
	"c_subti",  Argb8888,  256,  256; // misc
	"h_subti",  Argb8888,  512,  512;
	"c_mnbg01", Argb4444,  128,  128; // impossible to tell if 4444 or 1555
	"c_tuto20", Argb4444,  768,  512;
	"c_map00",  Argb1555, 1024,  512;
	"c_map00",  Argb1555,  768,  512;
	"c_map012", Argb1555, 1024,  512;
	"c_map01",  Argb4444, 1024,  512;
	"h_map01",  Argb4444, 2048, 1024;

	"c_camp01", Argb4444,  256,  256; // menu textures
	"h_camp01", Argb4444,  512,  512;
	"c_camp02", Argb1555,  256,  256;
	"h_camp02", Argb1555,  512,  512;
	"c_camp03", Argb1555,  256,  256;
	"h_camp03", Argb1555,  512,  512;
	"c_camp04", Argb4444,  256,  256;
	"h_camp04", Argb4444,  512,  512;
	"c_camp05", Argb4444,  256,  256;
	"h_camp05", Argb4444,  512,  512;

	"c_back",   Argb1555,  768,  512; // main menu bg (02 is for orbment menu)
	"w_back",   Argb1555, 1024,  512;
	"c_title0", Argb1555, 1024,  512;
	"c_title1", Argb4444,  512,  512;
	"h_title1", Argb4444, 1024, 1024;
	"c_title2", Argb4444, 1024,  512;
	"c_title3", Argb4444,  512,  512;
	"c_title4", Argb4444, 1024,  512;
	"c_title5", Argb1555, 1024,  512;
	"c_title6", Argb1555, 1024,  512;

	"c_book",   Argb1555,  768,  512;
	"c_cook",   Argb1555,  512,  512;
	"c_raback", Argb1555, 1024,  512;
	"c_encnt1", Argb1555,  768,  512;
	"c_gameov", Argb1555,  768,  512;

	"c_vis419", Argb4444,  768,  512;
	"c_vis438", Argb4444,  768,  512;
	"c_vis439", Argb4444,  768,  512;
	"h_vis419", Argb4444, 1536, 1024;
	"h_vis438", Argb4444, 1536, 1024;
	"h_vis439", Argb4444, 1536, 1024;
	"c_vis448", Argb4444,  768,  512;
	"c_vis478", Argb4444,  768,  512;
	"c_vis53",  Argb4444,  768,  512;
	"c_vis54",  Argb4444,  768,  512;

	"c_vis",    Argb1555,  768,  512;
	"c_vis",    Argb1555,  256,  256;
	"c_vis",    Argb1555,  512,  256;
	"c_vis",    Argb1555,  640,  304;
	"c_vis",    Argb1555,  128,   64;
	"c_vis",    Argb1555, 1024, 1024;
	"h_vis",    Argb1555, 1536, 1024;
	"w_vis",    Argb1555, 2048, 1024;

	"",         Argb4444,  256,  256;
	"",         Argb4444,  512,  512;
	"",         Argb4444,  768,  512;
	"",         Argb4444, 1024, 1024;
}

#[cfg(test)]
#[filetest::filetest("../../samples/ch/*._ch")]
fn test_parse_all(path: &std::path::Path, bytes: &[u8]) -> Result<(), anyhow::Error> {
	let name = path.file_name().unwrap().to_str().unwrap();
	let (mode, width, _) = guess_from_byte_size(name, bytes.len()).unwrap();
	let itp = read(mode, width, bytes)?;
	let bytes2 = write(&itp)?;
	let itp2 = read(mode, width, &bytes2)?;
	assert_eq!(itp, itp2);
	Ok(())
}
