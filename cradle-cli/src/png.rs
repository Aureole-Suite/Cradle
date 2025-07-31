use std::io::{Read, Write};

use cradle::raster::Raster;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Png {
	Argb32(Vec<Raster<u32>>),
	Indexed(Vec<u32>, Vec<Raster<u8>>),
}

impl Png {
	pub fn width(&self) -> usize {
		match self {
			Png::Argb32(d) => d[0].width(),
			Png::Indexed(_, d) => d[0].width(),
		}
	}

	pub fn height(&self) -> usize {
		match self {
			Png::Argb32(d) => d[0].height(),
			Png::Indexed(_, d) => d[0].height(),
		}
	}
}

pub fn write(w: impl Write, img: &Png) -> eyre::Result<()> {
	let mut png = png::Encoder::new(w, img.width() as u32, img.height() as u32);
	match img {
		Png::Argb32(data) => {
			png.set_color(png::ColorType::Rgba);
			png.set_depth(png::BitDepth::Eight);
			write_frames(data, png, |&argb| {
				let [b, g, r, a] = argb.to_le_bytes();
				[r, g, b, a]
			})
		}
		Png::Indexed(palette, data) => {
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

pub fn read(f: impl Read) -> eyre::Result<Png> {
	let png = png::Decoder::new(f).read_info()?;
	eyre::ensure!(
		png.info().bit_depth == png::BitDepth::Eight,
		"only 8-bit png is supported"
	);

	let pal = png.info().palette.as_ref().map(|pal| {
		let mut pal = pal
			.as_chunks().0.iter()
			.map(|&[r, g, b]| u32::from_le_bytes([b, g, r, 0xFF]))
			.collect::<Vec<_>>();
		if let Some(trns) = &png.info().trns {
			for (rgb, a) in pal.iter_mut().zip(trns.iter()) {
				*rgb = *rgb & 0xFFFFFF | (*a as u32) << 24;
			}
		}
		pal
	});

	Ok(match png.info().color_type {
		png::ColorType::Indexed => Png::Indexed(pal.unwrap(), read_frames(png, |[a]| a)?),
		png::ColorType::Grayscale => {
			Png::Argb32(read_frames(png, |[k]| u32::from_le_bytes([k, k, k, 0xFF]))?)
		}
		png::ColorType::GrayscaleAlpha => {
			Png::Argb32(read_frames(png, |[k, a]| u32::from_le_bytes([k, k, k, a]))?)
		}
		png::ColorType::Rgb => Png::Argb32(read_frames(png, |[r, g, b]| {
			u32::from_le_bytes([b, g, r, 0xFF])
		})?),
		png::ColorType::Rgba => Png::Argb32(read_frames(png, |[r, g, b, a]| {
			u32::from_le_bytes([b, g, r, a])
		})?),
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
				.as_chunks().0.iter()
				.copied()
				.map(&mut sample)
				.collect(),
		))
	}
	Ok(out)
}
