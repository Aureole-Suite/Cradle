pub mod dds;

pub fn to_dds(itp: &cradle::itp::Itp) -> (dds::Dds, Vec<u8>) {
	match &itp.data {
		cradle::itp::ImageData::Indexed(pal, data) => {
			let header = dds::Dds {
				flags: dds::DDSD::DEFAULT,
				height: itp.height,
				width: itp.width,
				pixel_format: dds::PixelFormat {
					flags: dds::DDPF::RGB | dds::DDPF::ALPHAPIXELS | dds::DDPF::PALETTEINDEXED8,
					rgb_bit_count: 8,
					..dds::PixelFormat::default()
				},
				..dds::Dds::default()
			};
			let cradle::itp::Palette::Embedded(pal) = pal else {
				panic!("external palette not supported");
			};
			let data = pal.iter()
				.flat_map(|a| u32::to_le_bytes(*a))
				.chain(data.iter().copied())
				.collect();
			(header, data)
		}
		cradle::itp::ImageData::Argb16_1(_) => todo!(),
		cradle::itp::ImageData::Argb16_2(_) => todo!(),
		cradle::itp::ImageData::Argb16_3(_) => todo!(),
		cradle::itp::ImageData::Argb32(_) => todo!(),
		cradle::itp::ImageData::Bc1(_) => todo!(),
		cradle::itp::ImageData::Bc2(_) => todo!(),
		cradle::itp::ImageData::Bc3(_) => todo!(),
		cradle::itp::ImageData::Bc7(_) => todo!(),
	}
}

#[cfg(test)]
#[test]
fn test_dds() -> anyhow::Result<()> {
	use std::io::Write;

	let path = "../samples/itp/ao_gf__c_vis289.itp";
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
