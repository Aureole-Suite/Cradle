use std::io::Write;

use cradle::itp::{Itp, ImageData, Palette};

use crate::Cli;

pub fn itp_to_png(cli: &Cli, f: impl Write, itp: &Itp) -> eyre::Result<()> {
	let Itp { status: _, width, height, ref data } = *itp;
	match data {
		ImageData::Indexed(pal, data) => {
			let pal = match pal {
				Palette::Embedded(pal) => pal,
				Palette::External(_) => eyre::bail!("external palette is not currently supported"),
			};
			if cli.png_no_palette {
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
	let len = (width * height) as usize;
	cradle::permute::unswizzle(&mut data[..len], height as usize, width as usize, 4, 4);
	write_png(w, width, height, &data)
}

fn write_png(
	mut w: impl Write,
	width: u32,
	height: u32,
	data: &[u32],
) -> eyre::Result<()> {
	let len = (width * height) as usize;
	if data.len() > len {
		tracing::warn!("discarding mipmaps")
	}
	let data = &data[..len];

	let mut png = png::Encoder::new(&mut w, width, height);
	png.set_color(png::ColorType::Rgba);
	png.set_depth(png::BitDepth::Eight);
	let mut w = png.write_header()?;
	let data: Vec<u8> = data.iter()
		.flat_map(|argb| {
			let [b, g, r, a] = argb.to_le_bytes();
			[r, g, b, a]
		})
		.collect::<Vec<_>>();
	w.write_image_data(&data)?;
	w.finish()?;
	Ok(())
}

fn write_indexed_png(
	mut w: impl Write,
	width: u32,
	height: u32,
	palette: &[u32],
	data: &[u8],
) -> eyre::Result<()> {
	let len = (width * height) as usize;
	if data.len() > len {
		tracing::warn!("discarding mipmaps")
	}
	let data = &data[..len];

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
	let mut w = png.write_header()?;
	w.write_image_data(data)?;
	w.finish()?;
	Ok(())
}
