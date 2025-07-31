use std::backtrace::Backtrace;

use gospel::read::{Le as _, Reader};

use crate::raster::Raster;
use crate::util::OptionTExt as _;

#[derive(Debug, thiserror::Error)]
pub enum ReadError {
	#[error("{source}")]
	Gospel {
		#[from]
		source: gospel::read::Error,
		backtrace: Backtrace,
	},
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

pub fn read(ch: &[u8], cp: &[u8]) -> Result<Vec<Raster<u32>>, ReadError> {
	let mut ch = Reader::new(ch);
	let n_tiles = ch.u16()? as usize;
	let mut tiles = Vec::with_capacity(n_tiles);
	for _ in 0..n_tiles {
		tiles.push(std::array::try_from_fn(|_| {
			ch.u16().map(crate::ch::from_4444)
		})?);
	}

	unduplicate(&mut tiles);

	let mut cp = Reader::new(cp);
	let n_frames = cp.u16()? as usize;
	let mut frames = Vec::with_capacity(n_frames);
	for _ in 0..n_frames {
		let mut img = Vec::with_capacity(256 * 256);
		for _ in 0..256 {
			img.extend(match cp.u16()? as usize {
				0xFFFF => &[0; 256],
				ix => tiles.get(ix).or_whatever("invalid tile id")?,
			})
		}
		frames.push(Raster::new_with(256, 256, img));
	}

	Ok(frames)
}

fn unduplicate(_tiles: &mut Vec<[u32; 256]>) {
	// TODO
}
