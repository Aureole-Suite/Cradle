use cradle::{
	itp::{ImageData, Itp, ItpRevision, Palette},
	raster::Raster,
};

use crate::{png, Args};

pub fn itp_to_png(args: &Args, itp: &Itp) -> eyre::Result<png::Png> {
	use {png::ImageData as PID, ImageData as ID};
	let data = match &itp.data {
		ID::Indexed(pal, data) => {
			let pal = match pal {
				Palette::Embedded(pal) => pal,
				Palette::External(_) => eyre::bail!("external palette is not currently supported"),
			};
			if args.png_no_palette {
				PID::Argb32(map(args, data, |i| i.map(|a| pal[*a as usize])))
			} else {
				PID::Indexed(pal.clone(), map(args, data, |i| i.clone()))
			}
		}
		ID::Argb16(_, _) => eyre::bail!("16-bit color is not currently supported"),
		ID::Argb32(data) => PID::Argb32(map(args, data, |i| i.clone())),
		ID::Bc1(data) => PID::Argb32(map(args, data, |i| decode(i, cradle_dxt::decode_bc1))),
		ID::Bc2(data) => PID::Argb32(map(args, data, |i| decode(i, cradle_dxt::decode_bc2))),
		ID::Bc3(data) => PID::Argb32(map(args, data, |i| decode(i, cradle_dxt::decode_bc3))),
		ID::Bc7(data) => PID::Argb32(map(args, data, |i| decode(i, cradle_dxt::decode_bc7))),
	};
	Ok(png::Png { data })
}

pub fn png_to_itp(args: &Args, png: &png::Png) -> Itp {
	let data = match &png.data {
		png::ImageData::Argb32(data) => ImageData::Argb32(map(args, data, |i| i.clone())),
		png::ImageData::Indexed(pal, data) if args.png_no_palette => {
			ImageData::Argb32(map(args, data, |i| {
				i.map(|a| *pal.get(*a as usize).unwrap_or(&0))
			}))
		}
		png::ImageData::Indexed(pal, data) => ImageData::Indexed(
			Palette::Embedded(pal.clone()),
			map(args, data, |i| i.clone()),
		),
	};
	Itp::new(ItpRevision::V3, data)
}

fn map<T, U>(
	args: &Args,
	mut data: &[Raster<T>],
	f: impl FnMut(&Raster<T>) -> Raster<U>,
) -> Vec<Raster<U>> {
	if data.len() > 1 && !args.png_mipmap {
		tracing::warn!("discarding mipmaps");
		data = &data[..1];
	}
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
	test_parse_all_inner(&Args::default(), bytes)
}

#[cfg(test)]
#[filetest::filetest("../../samples/itp/*.itp")]
fn test_parse_all_with_mip(bytes: &[u8]) -> Result<(), eyre::Error> {
	test_parse_all_inner(
		&Args {
			png_mipmap: true,
			..Args::default()
		},
		bytes,
	)
}

#[cfg(test)]
fn test_parse_all_inner(args: &Args, bytes: &[u8]) -> Result<(), eyre::Error> {
	use std::io::Cursor;
	let itp = cradle::itp::read(bytes)?;
	let png = itp_to_png(args, &itp)?;
	let itp2 = png_to_itp(args, &png);
	let png2 = itp_to_png(args, &itp2)?;
	assert_eq!(png, png2);

	let mut png_data = Vec::new();
	png::write(Cursor::new(&mut png_data), &png)?;
	let png2 = png::read(Cursor::new(&png_data))?;
	assert_eq!(png, png2);
	Ok(())
}
