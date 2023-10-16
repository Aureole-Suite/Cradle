use std::io::{Write, Read};

use cradle::itp::{Itp, ImageData, Palette, mipmaps};

use crate::Args;

pub fn itp_to_png(args: &Args, f: impl Write, itp: &Itp) -> eyre::Result<()> {
	let Itp { status: _, width, height, ref data } = *itp;
	match data {
		ImageData::Indexed(pal, data) => {
			let pal = match pal {
				Palette::Embedded(pal) => pal,
				Palette::External(_) => eyre::bail!("external palette is not currently supported"),
			};
			if args.png_no_palette {
				let data = data.iter().map(|a| pal[*a as usize]).collect::<Vec<_>>();
				write_png(args, f, width, height, &data)?;
			} else {
				write_indexed_png(args, f, width, height,pal, data)?;
			}
		},
		ImageData::Argb16(_, _) => eyre::bail!("16-bit color is not currently supported"),
		ImageData::Argb32(data) => write_png(args, f, width, height, data)?,
		ImageData::Bc1(data) => bc_to_png(args, f, width, height, data, cradle_dxt::decode_bc1)?,
		ImageData::Bc2(data) => bc_to_png(args, f, width, height, data, cradle_dxt::decode_bc2)?,
		ImageData::Bc3(data) => bc_to_png(args, f, width, height, data, cradle_dxt::decode_bc3)?,
		ImageData::Bc7(data) => bc_to_png(args, f, width, height, data, cradle_dxt::decode_bc7)?,
	}
	Ok(())
}

fn bc_to_png<T: Copy>(
	args: &Args,
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
			h as usize,
			w as usize,
			4,
			4,
		);
	}
	write_png(args, w, width, height, &data)
}

fn write_png(
	args: &Args,
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
	write_mips(args, png, width, height, &data, 4)?;
	Ok(())
}

fn write_indexed_png(
	args: &Args,
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
	write_mips(args, png, width, height, data, 1)?;
	Ok(())
}

fn write_mips<T: Write>(
	args: &Args,
	mut png: png::Encoder<T>,
	width: u32,
	height: u32,
	data: &[u8],
	bpp: usize,
) -> eyre::Result<()> {
	let nmips = mipmaps(width, height, data.len() / bpp).count();
	if nmips > 1 && args.png_mipmap {
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
		if nmips > 1 && !args.png_mipmap {
			tracing::warn!("discarding mipmaps");
			break
		}
	}
	png.finish()?;
	Ok(())
}

pub fn png_to_itp(args: &Args, f: impl Read) -> eyre::Result<Itp> {
	let png = png::Decoder::new(f).read_info()?;
	eyre::ensure!(png.info().bit_depth == png::BitDepth::Eight, "only 8-bit png is supported");

	let width = png.info().width;
	let height = png.info().height;

	let pal = png.info().palette.as_ref().map(|pal| {
		let mut pal = pal.array_chunks()
			.map(|&[r, g, b]| u32::from_le_bytes([b, g, r, 0xFF]))
			.collect::<Vec<_>>();
		if let Some(trns) = &png.info().trns {
			for (rgb, a) in pal.iter_mut().zip(trns.iter()) {
				*rgb = *rgb & 0xFFFFFF | (*a as u32) << 24;
			}
		}
		pal
	});

	let imgdata = match png.info().color_type {
		png::ColorType::Indexed if !args.png_no_palette =>
			ImageData::Indexed(Palette::Embedded(pal.unwrap()), read_frames(args, png, |[a]| a)?),
		png::ColorType::Indexed =>
			ImageData::Argb32(read_frames(args, png, |[i]| *pal.as_ref().unwrap().get(i as usize).unwrap_or(&0))?),
		png::ColorType::Grayscale =>
			ImageData::Argb32(read_frames(args, png, |[k]| u32::from_le_bytes([k, k, k, 0xFF]))?),
		png::ColorType::GrayscaleAlpha =>
			ImageData::Argb32(read_frames(args, png, |[k, a]| u32::from_le_bytes([k, k, k, a]))?),
		png::ColorType::Rgb =>
			ImageData::Argb32(read_frames(args, png, |[r, g, b]| u32::from_le_bytes([b, g, r, 0xFF]))?),
		png::ColorType::Rgba =>
			ImageData::Argb32(read_frames(args, png, |[r, g, b, a]| u32::from_le_bytes([b, g, r, a]))?),
	};

	Ok(Itp::new(cradle::itp::ItpRevision::V3, width, height, imgdata))
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
			break
		}
		let frame = png.next_frame(&mut buf)?;
		eyre::ensure!(frame.width == png.info().width >> n, "invalid frame width");
		eyre::ensure!(frame.height == png.info().height >> n, "invalid frame height");
		out.extend(
			buf[..frame.buffer_size()].array_chunks()
			.copied()
			.map(&mut sample)
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
	let mut png_data = Vec::new();
	itp_to_png(args, Cursor::new(&mut png_data), &itp)?;
	let itp2 = png_to_itp(args, Cursor::new(&png_data))?;
	let mut png_data2 = Vec::new();
	itp_to_png(args, Cursor::new(&mut png_data2), &itp2)?;
	assert!(png_data == png_data2);
	Ok(())
}
