use std::io::Write;

use cradle::itp::{Itp, ImageData, Palette, mipmaps};
use cradle_dds as dds;

pub fn itp_to_dds(mut write: impl Write, itp: &Itp) -> eyre::Result<()> {
	let Itp { status: _, width, height, ref data } = *itp;
	let mut header = dds::Dds { height, width, ..dds::Dds::default() };

	let nmip = mipmaps(width, height, data.pixel_count()).count();
	if nmip != 1 {
		header.flags |= dds::DDSD::MIPMAPCOUNT;
		header.mip_map_count = nmip as u32;
	}

	let data: Vec<u8> = match &data {
		ImageData::Indexed(pal, data) => {
			let pal = match pal {
				Palette::Embedded(pal) => pal,
				Palette::External(_) => eyre::bail!("external palette is not currently supported"),
			};
			header.pixel_format.flags |= dds::DDPF::PALETTEINDEXED8;
			header.pixel_format.rgb_bit_count = 8;
			let mut pal2 = [0; 256];
			pal2[..pal.len()].copy_from_slice(pal);
			pal2.iter()
				.flat_map(|a| u32::to_le_bytes(*a))
				.chain(data.iter().copied())
				.collect()
		}
		ImageData::Argb16_1(_) => eyre::bail!("16-bit color is not currently supported"),
		ImageData::Argb16_2(_) => eyre::bail!("16-bit color is not currently supported"),
		ImageData::Argb16_3(_) => eyre::bail!("16-bit color is not currently supported"),
		ImageData::Argb32(data) => {
			data.iter().copied()
				.flat_map(u32::to_le_bytes)
				.collect()
		}
		ImageData::Bc1(data) => {
			header.pixel_format.flags |= dds::DDPF::FOURCC;
			header.pixel_format.four_cc = *b"DXT1";
			data.iter().copied()
				.flat_map(u64::to_le_bytes)
				.collect()
		}
		ImageData::Bc2(data) => {
			header.pixel_format.flags |= dds::DDPF::FOURCC;
			header.pixel_format.four_cc = *b"DXT3";
			data.iter().copied()
				.flat_map(u128::to_le_bytes)
				.collect()
		}
		ImageData::Bc3(data) => {
			header.pixel_format.flags |= dds::DDPF::FOURCC;
			header.pixel_format.four_cc = *b"DXT5";
			data.iter().copied()
				.flat_map(u128::to_le_bytes)
				.collect()
		}
		ImageData::Bc7(data) => {
			header.pixel_format.flags |= dds::DDPF::FOURCC;
			header.pixel_format.four_cc = *b"DX10";
			header.dx10 = Some(dds::Dx10Header {
				dxgi_format: dds::DXGI_FORMAT::BC7_UNORM,
				..dds::Dx10Header::default()
			});
			data.iter().copied()
				.flat_map(u128::to_le_bytes)
				.collect()
		}
	};

	header.write(&mut write)?;
	write.write_all(&data)?;
	Ok(())
}
