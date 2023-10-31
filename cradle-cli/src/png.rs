use std::io::{Read, Write};

use cradle::raster::Raster;

use crate::Args;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Png {
	pub width: u32,
	pub height: u32,
	pub data: ImageData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageData {
	Argb32(Vec<Raster<u32>>),
	Indexed(Vec<u32>, Vec<Raster<u8>>),
}

pub fn write(args: &Args, w: impl Write, img: &Png) -> eyre::Result<()> {
	let mut png = png::Encoder::new(w, img.width, img.height);
	match &img.data {
		ImageData::Argb32(data) => {
			png.set_color(png::ColorType::Rgba);
			png.set_depth(png::BitDepth::Eight);
			write_frames(data, png, |&argb| {
				let [b, g, r, a] = argb.to_le_bytes();
				[r, g, b, a]
			})
		}
		ImageData::Indexed(palette, data) => {
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
			write_frames(data, png, |&i| [i])
		}
	}
}

fn write_frames<T, const N: usize>(
	data: &[Raster<T>],
	mut png: png::Encoder<impl Write>,
	mut f: impl FnMut(&T) -> [u8; N],
) -> Result<(), eyre::Error> {
	let nmips = data.len();
	if nmips > 1 {
		png.set_animated(nmips as u32, 0)?;
		png.set_frame_delay(1, 1)?;
		png.set_dispose_op(png::DisposeOp::Background)?;
	}
	let mut png = png.write_header()?;
	let mut first = true;
	for frame in data {
		if !std::mem::take(&mut first) {
			png.set_frame_dimension(frame.width() as u32, frame.height() as u32)?;
		}
		png.write_image_data(&frame.as_slice().iter().flat_map(&mut f).collect::<Vec<_>>())?;
	}
	png.finish()?;
	Ok(())
}

pub fn read(args: &Args, f: impl Read) -> eyre::Result<Png> {
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
		png::ColorType::Indexed => ImageData::Indexed(pal.unwrap(), read_frames(png, |[a]| a)?),
		png::ColorType::Grayscale => {
			ImageData::Argb32(read_frames(png, |[k]| u32::from_le_bytes([k, k, k, 0xFF]))?)
		}
		png::ColorType::GrayscaleAlpha => {
			ImageData::Argb32(read_frames(png, |[k, a]| u32::from_le_bytes([k, k, k, a]))?)
		}
		png::ColorType::Rgb => ImageData::Argb32(read_frames(png, |[r, g, b]| {
			u32::from_le_bytes([b, g, r, 0xFF])
		})?),
		png::ColorType::Rgba => ImageData::Argb32(read_frames(png, |[r, g, b, a]| {
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
	mut png: png::Reader<R>,
	mut sample: impl FnMut([u8; N]) -> T,
) -> eyre::Result<Vec<Raster<T>>> {
	let n_frames = png.info().animation_control.map_or(1, |ac| ac.num_frames);
	let mut buf = vec![0; png.output_buffer_size()];
	let mut out = Vec::new();
	for n in 0..n_frames {
		let frame = png.next_frame(&mut buf)?;
		eyre::ensure!(frame.width == png.info().width >> n, "invalid frame width");
		eyre::ensure!(
			frame.height == png.info().height >> n,
			"invalid frame height"
		);
		out.push(Raster::new_with(
			frame.width as usize,
			frame.height as usize,
			buf[..frame.buffer_size()]
				.array_chunks()
				.copied()
				.map(&mut sample)
				.collect(),
		))
	}
	Ok(out)
}
