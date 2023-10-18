use cradle::itp::{mipmaps, ImageData, Itp, ItpRevision, Palette};

use crate::{png, Args};

pub fn itp_to_png(args: &Args, itp: &Itp) -> eyre::Result<png::Png> {
	let Itp {
		status: _,
		width,
		height,
		ref data,
	} = *itp;
	use {png::ImageData as PID, ImageData as ID};
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
	Ok(png::Png {
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

pub fn png_to_itp(args: &Args, png: &png::Png) -> eyre::Result<Itp> {
	let png::Png {
		width,
		height,
		ref data,
	} = *png;
	let data = match data {
		png::ImageData::Argb32(data) => ImageData::Argb32(data.clone()),
		png::ImageData::Indexed(pal, data) if args.png_no_palette => ImageData::Argb32(
			data.iter()
				.map(|i| *pal.get(*i as usize).unwrap_or(&0))
				.collect(),
		),
		png::ImageData::Indexed(pal, data) => {
			ImageData::Indexed(Palette::Embedded(pal.clone()), data.clone())
		}
	};
	Ok(Itp::new(ItpRevision::V3, width, height, data))
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
	png::write(args, Cursor::new(&mut png_data), &png)?;
	let png2 = png::read(args, Cursor::new(&png_data))?;
	assert_eq!(png, png2);
	Ok(())
}
