use cradle::itp::{Itp, ImageData, Palette};

pub mod dds;

pub fn to_dds(itp: &Itp) -> Vec<u8> {
	let Itp { status: _, width, height, ref data } = *itp;
	let mut header = dds::Dds { height, width, ..dds::Dds::default() };
	let data: Vec<u8> = match &data {
		ImageData::Indexed(pal, data) => {
			let Palette::Embedded(pal) = pal else {
				panic!("external palette not supported");
			};
			header.pixel_format.flags |= dds::DDPF::PALETTEINDEXED8;
			header.pixel_format.rgb_bit_count = 8;
			set_mipmap(&mut header, data.len(), width * height);
			let mut pal2 = [0; 256];
			pal2[..pal.len()].copy_from_slice(pal);
			pal2.iter()
				.flat_map(|a| u32::to_le_bytes(*a))
				.chain(data.iter().copied())
				.collect()
		}
		ImageData::Argb16_1(_) => todo!(),
		ImageData::Argb16_2(_) => todo!(),
		ImageData::Argb16_3(_) => todo!(),
		ImageData::Argb32(data) => {
			set_mipmap(&mut header, data.len(), width * height);
			data.iter().copied()
				.flat_map(u32::to_le_bytes)
				.collect()
		}
		ImageData::Bc1(data) => {
			header.pixel_format.flags |= dds::DDPF::FOURCC;
			header.pixel_format.four_cc = *b"DXT1";
			set_mipmap(&mut header, data.len(), (width / 4) * (height / 4));
			data.iter().copied()
				.flat_map(u64::to_le_bytes)
				.collect()
		}
		ImageData::Bc2(data) => {
			header.pixel_format.flags |= dds::DDPF::FOURCC;
			header.pixel_format.four_cc = *b"DXT3";
			set_mipmap(&mut header, data.len(), (width / 4) * (height / 4));
			data.iter().copied()
				.flat_map(u128::to_le_bytes)
				.collect()
		}
		ImageData::Bc3(data) => {
			header.pixel_format.flags |= dds::DDPF::FOURCC;
			header.pixel_format.four_cc = *b"DXT5";
			set_mipmap(&mut header, data.len(), (width / 4) * (height / 4));
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
			set_mipmap(&mut header, data.len(), (width / 4) * (height / 4));
			data.iter().copied()
				.flat_map(u128::to_le_bytes)
				.collect()
		}
	};

	let mut dds = gospel::write::Writer::new();
	header.write(&mut dds);
	dds.slice(&data);
	dds.finish().unwrap()
}

fn set_mipmap(header: &mut dds::Dds, mut len: usize, imgsize: u32) {
	let mut imgsize = imgsize as usize;
	let mut nmip = 0;
	while imgsize >= len {
		len -= imgsize;
		imgsize /= 4;
		nmip += 1;
	}
	if nmip != 1 {
		header.flags |= dds::DDSD::MIPMAPCOUNT;
		header.mip_map_count = nmip;
	}
}
