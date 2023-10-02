pub mod dds;

pub fn to_dds(itp: &cradle::itp::Itp) -> (dds::Dds, Vec<u8>) {
	let mut header = dds::Dds {
		height: itp.height,
		width: itp.width,
		..dds::Dds::default()
	};
	let data = match &itp.data {
		cradle::itp::ImageData::Indexed(pal, data) => {
			let cradle::itp::Palette::Embedded(pal) = pal else {
				panic!("external palette not supported");
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
		cradle::itp::ImageData::Argb16_1(_) => todo!(),
		cradle::itp::ImageData::Argb16_2(_) => todo!(),
		cradle::itp::ImageData::Argb16_3(_) => todo!(),
		cradle::itp::ImageData::Argb32(data) => {
			data.iter().copied()
				.flat_map(u32::to_le_bytes)
				.collect()
		}
		cradle::itp::ImageData::Bc1(data) => {
			header.pixel_format.flags |= dds::DDPF::FOURCC;
			header.pixel_format.four_cc = *b"DXT1";
			data.iter().copied()
				.flat_map(u64::to_le_bytes)
				.collect()
		}
		cradle::itp::ImageData::Bc2(data) => {
			header.pixel_format.flags |= dds::DDPF::FOURCC;
			header.pixel_format.four_cc = *b"DXT3";
			data.iter().copied()
				.flat_map(u128::to_le_bytes)
				.collect()
		}
		cradle::itp::ImageData::Bc3(data) => {
			header.pixel_format.flags |= dds::DDPF::FOURCC;
			header.pixel_format.four_cc = *b"DXT5";
			data.iter().copied()
				.flat_map(u128::to_le_bytes)
				.collect()
		}
		cradle::itp::ImageData::Bc7(data) => {
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
	(header, data)
}

#[cfg(test)]
#[test]
fn test_dds() -> anyhow::Result<()> {
	use std::io::Write;

	let path = "../samples/itp/ys_celceta__f_00409.itp";
	let dat = std::fs::read(path)?;
	let itp = cradle::itp::read(&mut gospel::read::Reader::new(&dat))?;
	let (dds, data) = to_dds(&itp);
	let mut w = gospel::write::Writer::new();
	dds.write(&mut w);
	let mut f = std::fs::File::create("/tmp/a.dds")?;
	f.write_all(&w.finish().unwrap())?;
	f.write_all(&data)?;

	Ok(())
}
