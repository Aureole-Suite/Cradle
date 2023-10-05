use std::{io::Write, ops::Range};

use cradle::itp::{Itp, ImageData, Palette};

use crate::CLI;

pub fn itp_to_png(f: impl Write, itp: &Itp) -> eyre::Result<()> {
	let Itp { status: _, width, height, ref data } = *itp;
	match data {
		ImageData::Indexed(pal, data) => {
			let pal = match pal {
				Palette::Embedded(pal) => pal,
				Palette::External(_) => eyre::bail!("external palette is not currently supported"),
			};
			if CLI.png_no_palette {
				let data = data.iter().map(|a| pal[*a as usize]).collect::<Vec<_>>();
				write_png(f, width, height, &data)?;
			} else {
				write_indexed_png(f, width, height,pal, data)?;
			}
		},
		ImageData::Argb16_1(_) => eyre::bail!("16-bit color is not currently supported"),
		ImageData::Argb16_2(_) => eyre::bail!("16-bit color is not currently supported"),
		ImageData::Argb16_3(_) => eyre::bail!("16-bit color is not currently supported"),
		ImageData::Argb32(data) => write_png(f, width, height, data)?,
		ImageData::Bc1(data) => bc_to_png(f, width, height, data, cradle_dxt::decode_bc1)?,
		ImageData::Bc2(data) => bc_to_png(f, width, height, data, cradle_dxt::decode_bc2)?,
		ImageData::Bc3(data) => bc_to_png(f, width, height, data, cradle_dxt::decode_bc3)?,
		ImageData::Bc7(data) => bc_to_png(f, width, height, data, cradle_dxt::decode_bc7)?,
	}
	Ok(())
}

fn bc_to_png<T: Copy>(
	w: impl Write,
	width: u32,
	height: u32,
	data: &[T],
	f: impl FnMut(T) -> [u32; 16],
) -> eyre::Result<()> {
	let mut data = data.iter().copied().flat_map(f).collect::<Vec<_>>();
	for (w, h, range) in mipmaps(width, height, data.len()) {
		cradle::permute::unswizzle(
			&mut data[range],
			w as usize,
			h as usize,
			4,
			4,
		);
	}
	write_png(w, width, height, &data)
}

fn write_png(
	mut w: impl Write,
	width: u32,
	height: u32,
	data: &[u32],
) -> eyre::Result<()> {
	let mut png = png::Encoder::new(&mut w, width, height);
	png.set_color(png::ColorType::Rgba);
	png.set_depth(png::BitDepth::Eight);
	let data: Vec<u8> = data.iter()
		.flat_map(|argb| {
			let [b, g, r, a] = argb.to_le_bytes();
			[r, g, b, a]
		})
		.collect::<Vec<_>>();
	write_mips(png, width, height, &data, 4)?;
	Ok(())
}

fn write_indexed_png(
	mut w: impl Write,
	width: u32,
	height: u32,
	palette: &[u32],
	data: &[u8],
) -> eyre::Result<()> {
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
	write_mips(png, width, height, data, 1)?;
	Ok(())
}

fn write_mips<T: Write>(
	mut png: png::Encoder<T>,
	width: u32,
	height: u32,
	data: &[u8],
	bpp: usize,
) -> eyre::Result<()> {
	let nmips = mipmaps(width, height, data.len() / bpp).count();
	if nmips > 1 {
		png.set_animated(nmips as u32, 0)?;
		png.set_frame_delay(1, 1)?;
		png.set_dispose_op(png::DisposeOp::Background)?;
	}
	let mut png = png.write_header()?;
	let mut first = true;
	for (w, h, range) in mipmaps(width, height, data.len() / bpp) {
		if !std::mem::take(&mut first) {
			png.set_frame_dimension(w, h)?;
		}
		png.write_image_data(&data[range.start*bpp .. range.end*bpp])?;
		if nmips > 1 && !CLI.png_mipmap {
			tracing::warn!("discarding mipmaps");
			break
		}
	}
	png.finish()?;
	Ok(())
}

fn mipmaps(mut width: u32, mut height: u32, len: usize) -> impl Iterator<Item=(u32, u32, Range<usize>)> {
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
