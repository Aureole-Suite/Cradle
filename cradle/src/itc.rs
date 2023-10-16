use gospel::read::{Reader, Le as _};
use gospel::write::{Writer, Le as _};
use snafu::prelude::*;

#[derive(Debug, Snafu)]
pub enum ReadError {
	#[snafu(display("this is not an itc file"))]
	NotItc,

	#[allow(private_interfaces)]
	#[snafu(context(false))]
	Invalid { source: InnerReadError, backtrace: std::backtrace::Backtrace },
}

#[derive(Debug, Snafu)]
#[snafu(module(er), context(suffix(false)))]
enum InnerReadError {
	#[snafu(context(false))]
	Read { source: gospel::read::Error },

	#[snafu(context(false))]
	Compress { source: falcompress::Error },
}

impl From<gospel::read::Error> for ReadError {
	fn from(source: gospel::read::Error) -> Self {
		InnerReadError::from(source).into()
	}
}

impl From<falcompress::Error> for ReadError {
	fn from(source: falcompress::Error) -> Self {
		InnerReadError::from(source).into()
	}
}

#[derive(Debug, Snafu)]
pub enum WriteError {
	#[allow(private_interfaces)]
	#[snafu(context(false))]
	Invalid { source: InnerWriteError, backtrace: std::backtrace::Backtrace },
}

#[derive(Debug, Snafu)]
#[snafu(module(ew), context(suffix(false)))]
enum InnerWriteError {
	#[snafu(context(false))]
	Write { source: gospel::write::Error },
}

impl From<gospel::write::Error> for WriteError {
	fn from(source: gospel::write::Error) -> Self {
		InnerWriteError::from(source).into()
	}
}

macro_rules! bail {
	($e:expr) => { $e.fail::<!>()? }
}

#[derive(Clone, PartialEq)]
pub struct Frame {
	pub itp: Option<Vec<u8>>,
	pub unknown: u16,
	pub offset: (f32, f32),
	pub scale: (f32, f32),
	pub order: usize,
}

impl std::fmt::Debug for Frame {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		struct OpaqueVec(usize);
		impl std::fmt::Debug for OpaqueVec {
			fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
				write!(f, "[_; {}]", self.0)
			}
		}

		if self == &Frame::default() {
			write!(f, "Frame::default()")
		} else {
			f.debug_struct("Frame")
				.field("itp", &self.itp.as_ref().map(|a| OpaqueVec(a.len())))
				.field("unknown", &self.unknown)
				.field("offset", &self.offset)
				.field("scale", &self.scale)
				.field("order", &self.order)
				.finish()
		}
	}
}

impl Default for Frame {
	fn default() -> Self {
		Self {
			itp: None,
			unknown: 0,
			offset: (0.0, 0.0),
			scale: (1.0, 1.0),
			order: usize::MAX,
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct Itc {
	pub frames: [Frame; 128],
	pub palette: Option<Vec<u32>>,
}

impl Default for Itc {
	fn default() -> Self {
		Self {
			frames: std::array::from_fn(|_| Default::default()),
			palette: Default::default(),
		}
	}
}

pub fn read(data: &[u8]) -> Result<Itc, ReadError> {
	let mut f = Reader::new(data);

	let has_palette = match &f.array()? {
		b"V101" => false,
		b"V102" => true,
		_ => bail!(NotItcSnafu),
	};

	let mut frames = std::array::from_fn(|_| Frame::default());

	for frame in &mut frames {
		let start = f.u32()? as usize;
		let length = f.u32()? as usize;
		if (start, length) != (0, 0) {
			frame.order = start;
			frame.itp = Some(f.at(start)?.slice(length)?.to_vec());
		}
	}

	let mut starts = frames.iter().map(|a| a.order).collect::<Vec<_>>();
	starts.sort();
	for frame in &mut frames {
		if frame.itp.is_some() {
			frame.order = starts.binary_search(&frame.order).unwrap();
		}
	}

	for k in &mut frames { k.unknown  = f.u16()?; }
	for k in &mut frames { k.offset.0 = f.f32()?; }
	for k in &mut frames { k.offset.1 = f.f32()?; }
	for k in &mut frames { k.scale.0  = f.f32()?; }
	for k in &mut frames { k.scale.1  = f.f32()?; }

	let palette = if has_palette {
		let pal_size = f.u32()? as usize;
		let mut palette = Vec::with_capacity(pal_size);
		for _ in 0..pal_size {
			palette.push(f.u32()?);
		}
		Some(palette)
	} else {
		None
	};

	Ok(Itc {
		frames,
		palette,
	})
}

pub fn write(itc: &Itc) -> Result<Vec<u8>, WriteError> {
	let mut f = Writer::new();
	let mut slice = Writer::new();
	let mut unknown = Writer::new();
	let mut x_offset = Writer::new();
	let mut y_offset = Writer::new();
	let mut x_scale = Writer::new();
	let mut y_scale = Writer::new();
	let mut palette = Writer::new();

	if let Some(pal) = &itc.palette {
		f.slice(b"V102");
		palette.u32(pal.len() as u32);
		for c in pal {
			palette.u32(*c);
		}
	} else {
		f.slice(b"V101");
	}

	let mut outputs = Vec::new();

	for frame in &itc.frames {
		if let Some(itp) = &frame.itp {
			let mut g = Writer::new();
			slice.label32(g.here());
			g.slice(itp);
			slice.u32(g.len() as u32);
			outputs.push((frame.order, g));
		} else {
			slice.u32(0);
			slice.u32(0);
		}
		unknown.u16(frame.unknown);
		x_offset.f32(frame.offset.0);
		y_offset.f32(frame.offset.1);
		x_scale.f32(frame.scale.0);
		y_scale.f32(frame.scale.1);
	}

	f.append(slice);
	f.append(unknown);
	f.append(x_offset);
	f.append(y_offset);
	f.append(x_scale);
	f.append(y_scale);
	f.append(palette);

	outputs.sort_by_key(|a| a.0);
	for (_, output) in outputs {
		f.append(output)
	}

	Ok(f.finish()?)
}

#[cfg(test)]
#[filetest::filetest("../../samples/itc/*.itc")]
fn test_parse_all(bytes: &[u8]) -> Result<(), anyhow::Error> {
	let itc = read(bytes)?;
	let bytes2 = write(&itc)?;
	assert_eq!(bytes, bytes2);

	for frame in &itc.frames {
		if let Some(itpdata) = &frame.itp {
			let itp = crate::itp::read(itpdata)?;
			crate::itp::write(&itp)?;
		}
	}
	Ok(())
}
