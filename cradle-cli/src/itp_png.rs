use cradle::{
	itp::{ImageData, Itp, ItpRevision, Palette},
	raster::Raster,
};

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
				PID::Argb32(map(data, |i| i.map(|a| pal[*a as usize])))
			} else {
				PID::Indexed(pal.clone(), data.clone())
			}
		}
		ID::Argb16(_, _) => eyre::bail!("16-bit color is not currently supported"),
		ID::Argb32(data) => PID::Argb32(data.clone()),
		ID::Bc1(data) => PID::Argb32(map(data, |i| decode(i, cradle_dxt::decode_bc1))),
		ID::Bc2(data) => PID::Argb32(map(data, |i| decode(i, cradle_dxt::decode_bc2))),
		ID::Bc3(data) => PID::Argb32(map(data, |i| decode(i, cradle_dxt::decode_bc3))),
		ID::Bc7(data) => PID::Argb32(map(data, |i| decode(i, cradle_dxt::decode_bc7))),
	};
	Ok(png::Png {
		width,
		height,
		data,
	})
}

pub fn png_to_itp(args: &Args, png: &png::Png) -> Itp {
	let png::Png {
		width,
		height,
		ref data,
	} = *png;
	let data = match data {
		png::ImageData::Argb32(data) => ImageData::Argb32(data.clone()),
		png::ImageData::Indexed(pal, data) if args.png_no_palette => {
			ImageData::Argb32(map(data, |i| {
				i.map(|a| *pal.get(*a as usize).unwrap_or(&0))
			}))
		}
		png::ImageData::Indexed(pal, data) => {
			ImageData::Indexed(Palette::Embedded(pal.clone()), data.clone())
		}
	};
	Itp::new(ItpRevision::V3, width, height, data)
}

fn map<T, U>(data: &[T], f: impl FnMut(&T) -> U) -> Vec<U> {
	data.iter().map(f).collect()
}

fn decode<T: Copy>(r: &Raster<T>, f: impl FnMut(T) -> [u32; 16]) -> Raster<u32> {
	let mut data = r.as_slice().iter().copied().flat_map(f).collect::<Vec<_>>();
	cradle::permute::unswizzle(&mut data, r.height() * 4, r.width() * 4, 4, 4);
	Raster::new_with(r.width() * 4, r.height() * 4, data)
}

#[cfg(test)]
#[filetest::filetest("../../samples/itp/*.itp")]
fn test_parse_all(bytes: &[u8]) -> Result<(), eyre::Error> {
	let args = &Args::default();
	use std::io::Cursor;
	let itp = cradle::itp::read(bytes)?;
	let png = itp_to_png(args, &itp)?;
	let itp2 = png_to_itp(args, &png);
	let png2 = itp_to_png(args, &itp2)?;
	assert_eq!(png, png2);

	let mut png_data = Vec::new();
	png::write(args, Cursor::new(&mut png_data), &png)?;
	let png2 = png::read(args, Cursor::new(&png_data))?;
	assert_eq!(png, png2);
	Ok(())
}
