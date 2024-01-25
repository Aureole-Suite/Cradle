use std::io::Write;

use cradle::{
	ch::{ImageData, Mode},
	raster::Raster,
};

use crate::png;
use crate::Args;

pub fn ch_to_png(args: &Args, ch: &ImageData) -> eyre::Result<png::Png> {
	let _ = args;
	Ok(match ch {
		ImageData::Argb1555(img) => png::Png::Argb32(vec![img.map(from_1555)]),
		ImageData::Argb4444(img) => png::Png::Argb32(vec![img.map(from_4444)]),
		ImageData::Argb8888(img) => png::Png::Argb32(vec![img.clone()]),
	})
}

fn from(a: u16, m: u16) -> u8 {
	let m = (1 << m) - 1;
	((a & m) * 255 / m) as u8
}

fn from_1555(&k: &u16) -> u32 {
	u32::from_le_bytes([
		from(k, 5),
		from(k >> 5, 5),
		from(k >> 10, 5),
		from(k >> 15, 1),
	])
}

fn from_4444(&k: &u16) -> u32 {
	u32::from_le_bytes([
		from(k, 4),
		from(k >> 4, 4),
		from(k >> 8, 4),
		from(k >> 12, 4),
	])
}

fn to(k: u8, m: u16) -> u16 {
	let m = (1 << m) - 1;
	(k as u16 * m + 128) / 255
}

fn to_1555(&k: &u32) -> u16 {
	let [b, g, r, a] = k.to_le_bytes();
	to(b, 5) | to(g, 5) << 5 | to(r, 5) << 10 | to(a, 1) << 15
}

fn to_4444(&k: &u32) -> u16 {
	let [b, g, r, a] = k.to_le_bytes();
	to(b, 4) | to(g, 4) << 4 | to(r, 4) << 8 | to(a, 4) << 12
}

pub fn itp_to_ch(args: &Args, mode: Mode, itp: &cradle::itp::Itp) -> eyre::Result<ImageData> {
	let _ = args;
	use cradle::itp;
	let data = match &itp.data {
		itp::ImageData::Argb32(data) if data.len() == 1 => data[0].clone(),
		itp::ImageData::Indexed(itp::Palette::Embedded(pal), data) if data.len() == 1 => {
			data[0].map(|a| *pal.get(*a as usize).unwrap_or(&0))
		}
		_ => eyre::bail!("unsupported input format"),
	};
	Ok(match mode {
		Mode::Argb1555 => ImageData::Argb1555(data.map(to_1555)),
		Mode::Argb4444 => ImageData::Argb4444(data.map(to_4444)),
		Mode::Argb8888 => ImageData::Argb8888(data),
	})
}

pub fn ch_to_dds(args: &Args, write: impl Write, ch: &ImageData) -> eyre::Result<()> {
	let _ = args;
	match ch {
		ImageData::Argb1555(img) => to_dds(
			write,
			img,
			u16::to_le_bytes,
			[0x8000, 0x7C00, 0x03E0, 0x001F],
		),
		ImageData::Argb4444(img) => to_dds(
			write,
			img,
			u16::to_le_bytes,
			[0xF000, 0x0F00, 0x00F0, 0x000F],
		),
		ImageData::Argb8888(img) => to_dds(
			write,
			img,
			u32::to_le_bytes,
			[0xFF000000, 0x00FF0000, 0x0000FF00, 0x000000FF],
		),
	}
}

fn to_dds<T: Copy, const N: usize>(
	mut write: impl Write,
	data: &Raster<T>,
	to_le_bytes: fn(T) -> [u8; N],
	mask: [u32; 4],
) -> eyre::Result<()> {
	let mut header = cradle_dds::Dds {
		width: data.width() as u32,
		height: data.height() as u32,
		..cradle_dds::Dds::default()
	};
	[
		header.pixel_format.amask,
		header.pixel_format.rmask,
		header.pixel_format.gmask,
		header.pixel_format.bmask,
	] = mask;
	header.pixel_format.bpp = 8 * N as u32;
	header.write(&mut write)?;
	let data = data
		.as_slice()
		.iter()
		.flat_map(|v| to_le_bytes(*v))
		.collect::<Vec<_>>();
	write.write_all(&data)?;
	Ok(())
}

#[cfg(test)]
#[filetest::filetest("../../samples/ch/*._ch")]
fn test_parse_all_dds(path: &camino::Utf8Path, bytes: &[u8]) -> Result<(), eyre::Error> {
	let args = &Args::default();
	use std::io::Cursor;
	let (mode, width, _) =
		cradle::ch::guess_from_byte_size(path.file_name().unwrap(), bytes.len()).unwrap();
	let ch = cradle::ch::read(mode, width, bytes)?;
	let mut ch_data = Vec::new();
	ch_to_dds(args, Cursor::new(&mut ch_data), &ch)?;
	let itp = crate::itp_dds::dds_to_itp(args, Cursor::new(&ch_data))?;
	let ch2 = itp_to_ch(args, mode, &itp)?;
	assert_eq!(ch, ch2);
	let mut dds_data2 = Vec::new();
	ch_to_dds(args, Cursor::new(&mut dds_data2), &ch2)?;
	assert!(ch_data == dds_data2);
	Ok(())
}

#[cfg(test)]
#[filetest::filetest("../../samples/ch/*._ch")]
fn test_parse_all_png(path: &camino::Utf8Path, bytes: &[u8]) -> Result<(), eyre::Error> {
	let args = &Args::default();
	let (mode, width, _) =
		cradle::ch::guess_from_byte_size(path.file_name().unwrap(), bytes.len()).unwrap();
	let ch = cradle::ch::read(mode, width, bytes)?;
	let png = ch_to_png(args, &ch)?;
	let itp = crate::itp_png::png_to_itp(args, &png);
	let ch2 = itp_to_ch(args, mode, &itp)?;
	assert_eq!(ch, ch2);
	let png2 = ch_to_png(args, &ch2)?;
	assert!(png == png2);
	Ok(())
}
