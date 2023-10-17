use std::io::{Read, Write};

use cradle::itp::{mipmaps, ImageData, Itp, ItpRevision, Palette};

use crate::Args;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Png {
	pub width: u32,
	pub height: u32,
	pub data: PngImageData,
}

#[derive(Clone, PartialEq, Eq)]
pub enum PngImageData {
	Argb32(Vec<u32>),
	Indexed(Vec<u32>, Vec<u8>),
}

impl std::fmt::Debug for PngImageData {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Argb32(data) => f.debug_tuple("Argb32").field(&data.len()).finish(),
			Self::Indexed(pal, data) => f
				.debug_tuple("Indexed")
				.field(pal)
				.field(&data.len())
				.finish(),
		}
	}
}

pub fn itp_to_png(args: &Args, itp: &Itp) -> eyre::Result<Png> {
	let Itp {
		status: _,
		width,
		height,
		ref data,
	} = *itp;
	use {ImageData as ID, PngImageData as PID};
	let data = match data {
		ID::Indexed(pal, data) => {
			let pal = match pal {
				Palette::Embedded(pal) => pal,
				Palette::External(_) => eyre::bail!("external palette is not currently supported"),
			};
			if args.png_no_palette {
				PID::Argb32(data.iter().map(|a| pal[*a as usize]).collect::<Vec<_>>())
			} else {
				PID::Indexed(pal.clone(), data.clone())
			}
		}
		ID::Argb16(_, _) => eyre::bail!("16-bit color is not currently supported"),
		ID::Argb32(data) => PID::Argb32(data.clone()),
		ID::Bc1(data) => PID::Argb32(decode(width, height, data, cradle_dxt::decode_bc1)),
		ID::Bc2(data) => PID::Argb32(decode(width, height, data, cradle_dxt::decode_bc2)),
		ID::Bc3(data) => PID::Argb32(decode(width, height, data, cradle_dxt::decode_bc3)),
		ID::Bc7(data) => PID::Argb32(decode(width, height, data, cradle_dxt::decode_bc7)),
	};
	Ok(Png {
		width,
		height,
		data,
	})
}

fn decode<T: Copy>(width: u32, height: u32, data: &[T], f: impl FnMut(T) -> [u32; 16]) -> Vec<u32> {
	let mut data = data.iter().copied().flat_map(f).collect::<Vec<_>>();
	for (w, h, range) in mipmaps(width, height, data.len()) {
		cradle::permute::unswizzle(&mut data[range], h as usize, w as usize, 4, 4);
	}
	data
}

pub fn write_png(args: &Args, w: impl Write, img: &Png) -> eyre::Result<()> {
	let mut png = png::Encoder::new(w, img.width, img.height);
	let _data;
	let (data, bpp) = match &img.data {
		PngImageData::Argb32(data) => {
			_data = data
				.iter()
				.flat_map(|argb| {
					let [b, g, r, a] = argb.to_le_bytes();
					[r, g, b, a]
				})
				.collect::<Vec<_>>();
			png.set_color(png::ColorType::Rgba);
			png.set_depth(png::BitDepth::Eight);
			(&_data, 4)
		}
		PngImageData::Indexed(palette, data) => {
			let mut pal = Vec::with_capacity(3 * palette.len());
			let mut alp = Vec::with_capacity(palette.len());
			for argb in palette {
				let [b, g, r, a] = u32::to_le_bytes(*argb);
				pal.push(r);
				pal.push(g);
				pal.push(b);
				alp.push(a);
			}
			png.set_color(png::ColorType::Indexed);
			png.set_depth(png::BitDepth::Eight);
			png.set_palette(pal);
			png.set_trns(alp);
			(data, 1)
		}
	};

	let nmips = mipmaps(img.width, img.height, data.len() / bpp).count();
	if nmips > 1 && args.png_mipmap {
		png.set_animated(nmips as u32, 0)?;
		png.set_frame_delay(1, 1)?;
		png.set_dispose_op(png::DisposeOp::Background)?;
	}
	let mut png = png.write_header()?;
	let mut first = true;
	for (w, h, range) in mipmaps(img.width, img.height, data.len() / bpp) {
		if !std::mem::take(&mut first) {
			png.set_frame_dimension(w, h)?;
		}
		png.write_image_data(&data[range.start * bpp..range.end * bpp])?;
		if nmips > 1 && !args.png_mipmap {
			tracing::warn!("discarding mipmaps");
			break;
		}
	}
	png.finish()?;
	Ok(())
}

pub fn png_to_itp(args: &Args, png: &Png) -> eyre::Result<Itp> {
	let Png {
		width,
		height,
		ref data,
	} = *png;
	let data = match data {
		PngImageData::Argb32(data) => ImageData::Argb32(data.clone()),
		PngImageData::Indexed(pal, data) if args.png_no_palette => ImageData::Argb32(
			data.iter()
				.map(|i| *pal.get(*i as usize).unwrap_or(&0))
				.collect(),
		),
		PngImageData::Indexed(pal, data) => {
			ImageData::Indexed(Palette::Embedded(pal.clone()), data.clone())
		}
	};
	Ok(Itp::new(ItpRevision::V3, width, height, data))
}

pub fn read_png(args: &Args, f: impl Read) -> eyre::Result<Png> {
	let png = png::Decoder::new(f).read_info()?;
	eyre::ensure!(
		png.info().bit_depth == png::BitDepth::Eight,
		"only 8-bit png is supported"
	);

	let width = png.info().width;
	let height = png.info().height;

	let pal = png.info().palette.as_ref().map(|pal| {
		let mut pal = pal
			.array_chunks()
			.map(|&[r, g, b]| u32::from_le_bytes([b, g, r, 0xFF]))
			.collect::<Vec<_>>();
		if let Some(trns) = &png.info().trns {
			for (rgb, a) in pal.iter_mut().zip(trns.iter()) {
				*rgb = *rgb & 0xFFFFFF | (*a as u32) << 24;
			}
		}
		pal
	});

	let data = match png.info().color_type {
		png::ColorType::Indexed if !args.png_no_palette => {
			PngImageData::Indexed(pal.unwrap(), read_frames(args, png, |[a]| a)?)
		}
		png::ColorType::Indexed => PngImageData::Argb32(read_frames(args, png, |[i]| {
			*pal.as_ref().unwrap().get(i as usize).unwrap_or(&0)
		})?),
		png::ColorType::Grayscale => PngImageData::Argb32(read_frames(args, png, |[k]| {
			u32::from_le_bytes([k, k, k, 0xFF])
		})?),
		png::ColorType::GrayscaleAlpha => PngImageData::Argb32(read_frames(args, png, |[k, a]| {
			u32::from_le_bytes([k, k, k, a])
		})?),
		png::ColorType::Rgb => PngImageData::Argb32(read_frames(args, png, |[r, g, b]| {
			u32::from_le_bytes([b, g, r, 0xFF])
		})?),
		png::ColorType::Rgba => PngImageData::Argb32(read_frames(args, png, |[r, g, b, a]| {
			u32::from_le_bytes([b, g, r, a])
		})?),
	};

	Ok(Png {
		width,
		height,
		data,
	})
}

fn read_frames<R: Read, T, const N: usize>(
	args: &Args,
	mut png: png::Reader<R>,
	mut sample: impl FnMut([u8; N]) -> T,
) -> eyre::Result<Vec<T>> {
	let n_frames = png.info().animation_control.map_or(1, |ac| ac.num_frames);
	let mut buf = vec![0; png.output_buffer_size()];
	let mut out = Vec::new();
	for n in 0..n_frames {
		if n > 1 && !args.png_mipmap {
			tracing::warn!("discarding mipmaps");
			break;
		}
		let frame = png.next_frame(&mut buf)?;
		eyre::ensure!(frame.width == png.info().width >> n, "invalid frame width");
		eyre::ensure!(
			frame.height == png.info().height >> n,
			"invalid frame height"
		);
		out.extend(
			buf[..frame.buffer_size()]
				.array_chunks()
				.copied()
				.map(&mut sample),
		)
	}
	Ok(out)
}

#[cfg(test)]
#[filetest::filetest("../../samples/itp/*.itp")]
fn test_parse_all(bytes: &[u8]) -> Result<(), eyre::Error> {
	let args = &Args::default();
	use std::io::Cursor;
	let itp = cradle::itp::read(bytes)?;
	let png = itp_to_png(args, &itp)?;
	let itp2 = png_to_itp(args, &png)?;
	let png2 = itp_to_png(args, &itp2)?;
	assert_eq!(png, png2);

	let mut png_data = Vec::new();
	write_png(args, Cursor::new(&mut png_data), &png)?;
	let png2 = read_png(args, Cursor::new(&png_data))?;
	assert_eq!(png, png2);
	Ok(())
}
