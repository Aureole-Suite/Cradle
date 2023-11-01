use std::io::{Read, Write};

use cradle::{
	itp::{ImageData, Itp, ItpRevision, Palette},
	raster::Raster,
};
use cradle_dds as dds;

use strength_reduce::StrengthReducedU64 as SR64;

use crate::Args;

pub fn itp_to_dds(args: &Args, mut write: impl Write, itp: &Itp) -> eyre::Result<()> {
	let _ = args;
	let Itp {
		status: _,
		width,
		height,
		ref data,
	} = *itp;
	let mut header = dds::Dds {
		width: width as u32,
		height: height as u32,
		..dds::Dds::default()
	};

	let nmip = data.mipmaps();
	if nmip != 1 {
		header.flags |= dds::DDSD::MIPMAPCOUNT;
		header.mip_map_count = nmip as u32;
	}

	match &data {
		ImageData::Indexed(pal, data) => {
			let pal = match pal {
				Palette::Embedded(pal) => pal,
				Palette::External(_) => eyre::bail!("external palette is not currently supported"),
			};
			header.pixel_format.flags |= dds::DDPF::PALETTEINDEXED8;
			header.pixel_format.bpp = 8;
			header.write(&mut write)?;
			let mut pal2 = [0; 256];
			pal2[..pal.len()].copy_from_slice(pal);
			write.write_all(
				&pal2
					.iter()
					.flat_map(|a| {
						let [b, g, r, a] = u32::to_le_bytes(*a);
						[r, g, b, a]
					})
					.collect::<Vec<_>>(),
			)?;
			write_data(write, data, u8::to_le_bytes)
		}
		ImageData::Argb16(_, _) => eyre::bail!("16-bit color is not currently supported"),
		ImageData::Argb32(data) => {
			header.write(&mut write)?;
			write_data(write, data, u32::to_le_bytes)
		}
		ImageData::Bc1(data) => {
			header.pixel_format.flags |= dds::DDPF::FOURCC;
			header.pixel_format.four_cc = *b"DXT1";
			header.write(&mut write)?;
			write_data(write, data, u64::to_le_bytes)
		}
		ImageData::Bc2(data) => {
			header.pixel_format.flags |= dds::DDPF::FOURCC;
			header.pixel_format.four_cc = *b"DXT3";
			header.write(&mut write)?;
			write_data(write, data, u128::to_le_bytes)
		}
		ImageData::Bc3(data) => {
			header.pixel_format.flags |= dds::DDPF::FOURCC;
			header.pixel_format.four_cc = *b"DXT5";
			header.write(&mut write)?;
			write_data(write, data, u128::to_le_bytes)
		}
		ImageData::Bc7(data) => {
			header.pixel_format.flags |= dds::DDPF::FOURCC;
			header.pixel_format.four_cc = *b"DX10";
			header.dx10 = Some(dds::Dx10Header {
				dxgi_format: dds::DXGI_FORMAT::BC7_UNORM,
				..dds::Dx10Header::default()
			});
			header.write(&mut write)?;
			write_data(write, data, u128::to_le_bytes)
		}
	}
}

pub fn dds_to_itp(args: &Args, mut read: impl Read) -> eyre::Result<Itp> {
	let _ = args;
	let mut dds = dds::Dds::read(&mut read)?;
	un_dxgi(&mut dds);
	let pf = &dds.pixel_format;
	let data = if pf.flags & dds::DDPF::PALETTEINDEXED8 != 0 {
		let mut palette = [0; 4 * 256];
		read.read_exact(&mut palette)?;
		let mut palette = palette
			.array_chunks()
			.copied()
			.map(|[r, g, b, a]| u32::from_le_bytes([b, g, r, a]))
			.collect::<Vec<_>>();
		let data = read_data(read, &dds, 1, u8::from_le_bytes)?;

		let max = data
			.iter()
			.flat_map(|a| a.as_slice())
			.map(|a| *a as usize + 1)
			.max()
			.unwrap_or_default();
		while palette.len() > max && palette.last() == Some(&0) {
			palette.pop();
		}

		ImageData::Indexed(Palette::Embedded(palette), data)
	} else if pf.flags & dds::DDPF::FOURCC != 0 {
		match &pf.four_cc {
			b"DXT1" => ImageData::Bc1(read_data(read, &dds, 4, u64::from_le_bytes)?),
			b"DXT3" => ImageData::Bc2(read_data(read, &dds, 4, u128::from_le_bytes)?),
			b"DXT5" => ImageData::Bc3(read_data(read, &dds, 4, u128::from_le_bytes)?),
			b"DX10" => {
				let dx10 = dds.dx10.as_ref().unwrap();
				use dds::DXGI_FORMAT as D;
				match dx10.dxgi_format {
					D::BC1_TYPELESS | D::BC1_UNORM | D::BC1_UNORM_SRGB => {
						ImageData::Bc1(read_data(read, &dds, 4, u64::from_le_bytes)?)
					}
					D::BC2_TYPELESS | D::BC2_UNORM | D::BC2_UNORM_SRGB => {
						ImageData::Bc2(read_data(read, &dds, 4, u128::from_le_bytes)?)
					}
					D::BC3_TYPELESS | D::BC3_UNORM | D::BC3_UNORM_SRGB => {
						ImageData::Bc3(read_data(read, &dds, 4, u128::from_le_bytes)?)
					}
					D::BC7_TYPELESS | D::BC7_UNORM | D::BC7_UNORM_SRGB => {
						ImageData::Bc7(read_data(read, &dds, 4, u128::from_le_bytes)?)
					}
					_ => eyre::bail!("I don't understand this dds (dxgi)"),
				}
			}
			_ => eyre::bail!("I don't understand this dds (fourcc)"),
		}
	} else if pf.flags & dds::DDPF::RGB != 0 {
		let cmask = (
			sr64(pf.rmask),
			sr64(pf.gmask),
			sr64(pf.bmask),
			sr64(pf.amask),
		);
		match pf.bpp {
			32 => ImageData::Argb32(read_data(read, &dds, 1, |d| {
				mask(cmask, u32::from_le_bytes(d))
			})?),
			16 => ImageData::Argb32(read_data(read, &dds, 1, |d| {
				mask(cmask, u16::from_le_bytes(d) as u32)
			})?),
			8 => ImageData::Argb32(read_data(read, &dds, 1, |d| {
				mask(cmask, u8::from_le_bytes(d) as u32)
			})?),
			_ => eyre::bail!("I don't understand this dds (bbp)"),
		}
	} else {
		eyre::bail!("I don't understand this dds")
	};

	Ok(Itp::new(
		ItpRevision::V3,
		dds.width as usize,
		dds.height as usize,
		data,
	))
}

fn write_data<T: Copy, const N: usize>(
	mut write: impl Write,
	data: &[Raster<T>],
	to_le_bytes: impl FnMut(T) -> [u8; N],
) -> eyre::Result<()> {
	Ok(write.write_all(
		&data
			.iter()
			.flat_map(|a| a.as_slice())
			.copied()
			.flat_map(to_le_bytes)
			.collect::<Vec<_>>(),
	)?)
}

fn read_data<T, const N: usize>(
	mut read: impl Read,
	dds: &dds::Dds,
	scale: usize,
	mut from_le_bytes: impl FnMut([u8; N]) -> T,
) -> eyre::Result<Vec<Raster<T>>> {
	let mut out = Vec::new();
	for i in 0..dds.mip_map_count as usize {
		let w = (dds.width as usize >> i) / scale;
		let h = (dds.height as usize >> i) / scale;
		let mut data = vec![0; w * h * N];
		read.read_exact(&mut data)?;
		out.push(Raster::new_with(
			w,
			h,
			data.array_chunks().map(|a| from_le_bytes(*a)).collect(),
		))
	}
	Ok(out)
}

fn un_dxgi(dds: &mut dds::Dds) {
	let pf = &mut dds.pixel_format;
	if pf.flags & dds::DDPF::FOURCC != 0 && pf.four_cc == *b"DX10" {
		let dx10 = dds.dx10.as_ref().unwrap();
		use dds::DXGI_FORMAT as D;
		// Stole this table from Gimp
		let mask = match dx10.dxgi_format {
			D::B8G8R8A8_TYPELESS | D::B8G8R8A8_UNORM | D::B8G8R8A8_UNORM_SRGB => {
				(32, 0x00FF0000, 0x0000FF00, 0x000000FF, 0xFF000000)
			}
			D::B8G8R8X8_TYPELESS | D::B8G8R8X8_UNORM | D::B8G8R8X8_UNORM_SRGB => {
				(32, 0x00FF0000, 0x0000FF00, 0x000000FF, 0x00000000)
			}
			D::R8G8B8A8_TYPELESS
			| D::R8G8B8A8_UNORM
			| D::R8G8B8A8_UNORM_SRGB
			| D::R8G8B8A8_UINT
			| D::R8G8B8A8_SNORM
			| D::R8G8B8A8_SINT => (32, 0x000000FF, 0x0000FF00, 0x00FF0000, 0xFF000000),
			D::B5G6R5_UNORM => (16, 0xF800, 0x07E0, 0x001F, 0x0000),
			D::B5G5R5A1_UNORM => (16, 0x7C00, 0x03E0, 0x001F, 0x8000),
			D::R10G10B10A2_TYPELESS | D::R10G10B10A2_UNORM | D::R10G10B10A2_UINT => {
				(32, 0x000003FF, 0x000FFC00, 0x3FF00000, 0xC0000000)
			}
			D::A8_UNORM => (8, 0x00, 0x00, 0x00, 0xFF),
			D::R8_TYPELESS | D::R8_UNORM | D::R8_UINT | D::R8_SNORM | D::R8_SINT => {
				(8, 0xFF, 0x00, 0x00, 0x00)
			}
			D::B4G4R4A4_UNORM => (16, 0x0F00, 0x00F0, 0x000F, 0xF000),
			_ => return,
		};
		(pf.bpp, pf.rmask, pf.gmask, pf.bmask, pf.amask) = mask;
		pf.flags &= !dds::DDPF::FOURCC;
	}
}

fn mask((r, g, b, a): (SR64, SR64, SR64, SR64), x: u32) -> u32 {
	u32::from_le_bytes([mask1(b, x), mask1(g, x), mask1(r, x), mask1(a, x)])
}

fn sr64(mask: u32) -> SR64 {
	if mask == 0 {
		SR64::new(u64::MAX)
	} else {
		SR64::new(mask as u64)
	}
}

fn mask1(mask: SR64, x: u32) -> u8 {
	(((x as u64 & mask.get()) << 8).saturating_sub(1) / mask) as u8
}

#[test]
fn test_mask() {
	assert_eq!(mask1(sr64(0xF000), 0x1234), 0x11);
	assert_eq!(mask1(sr64(0b011100000), 0b001100000), 0b01101101);
	assert_eq!(mask1(sr64(0x00FE0000), 0xFEFEFEFE), 0xFF);
	assert_eq!(mask1(sr64(0xFE000000), 0xFEFEFEFE), 0xFF);
	assert_eq!(
		mask1(
			sr64(0b00000111111111100000000000000000),
			0b11111110111111101111111011111110,
		),
		0b11011111
	);
}

#[cfg(test)]
#[filetest::filetest("../../samples/itp/*.itp")]
fn test_parse_all(bytes: &[u8]) -> Result<(), eyre::Error> {
	let args = &Args::default();
	use std::io::Cursor;
	let itp = cradle::itp::read(bytes)?;
	let mut dds_data = Vec::new();
	itp_to_dds(args, Cursor::new(&mut dds_data), &itp)?;
	let itp2 = dds_to_itp(args, Cursor::new(&dds_data))?;
	assert_eq!(itp.data, itp2.data);
	let mut dds_data2 = Vec::new();
	itp_to_dds(args, Cursor::new(&mut dds_data2), &itp2)?;
	assert!(dds_data == dds_data2);
	Ok(())
}
