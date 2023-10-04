mod enums;
use std::io::{Read, Write, Result, Error, ErrorKind};

pub use enums::*;

#[derive(Debug, Clone)]
pub struct Dds {
	/// See [`DDSD`] for values.
	pub flags: u32,
	pub height: u32,
	pub width: u32,
	pub pitch: u32,
	pub depth: u32,
	pub mip_map_count: u32,
	pub reserved: [u32; 11],
	pub pixel_format: PixelFormat,
	/// See [`DDSCAPS`] for values.
	pub caps: u128,
	pub reserved2: u32,
	pub dx10: Option<Dx10Header>,
}

impl Dds {
	pub fn read(f: &mut impl Read) -> Result<Self> {
		f.check_u32(u32::from_le_bytes(*b"DDS "))?;
		f.check_u32(124)?;
		let flags = f.u32()?;
		let height = f.u32()?;
		let width = f.u32()?;
		let pitch = f.u32()?;
		let depth = f.u32()?;
		let mip_map_count = f.u32()?;
		let reserved = [
			f.u32()?, f.u32()?, f.u32()?, f.u32()?,
			f.u32()?, f.u32()?, f.u32()?, f.u32()?,
			f.u32()?, f.u32()?, f.u32()?,
		];
		let pixel_format = PixelFormat::read(f)?;
		let caps = f.u128()?;
		let reserved2 = f.u32()?;

		let dx10 = if pixel_format.four_cc == *b"DX10" {
			Some(Dx10Header::read(f)?)
		} else {
			None
		};

		Ok(Dds {
			flags, height, width, pitch, depth,
			mip_map_count, reserved, pixel_format,
			caps, reserved2, dx10,
		})
	}

	pub fn write(&self, f: &mut impl Write) -> Result<()> {
		f.write_all(b"DDS ")?;
		f.u32(124)?;
		f.u32(self.flags)?;
		f.u32(self.height)?;
		f.u32(self.width)?;
		f.u32(self.pitch)?;
		f.u32(self.depth)?;
		f.u32(self.mip_map_count)?;
		for v in self.reserved {
			f.u32(v)?;
		}
		self.pixel_format.write(f)?;
		f.u128(self.caps)?;
		f.u32(self.reserved2)?;

		if let Some(dx10) = &self.dx10 {
			assert_eq!(self.pixel_format.four_cc, *b"DX10");
			dx10.write(f)?;
		} else {
			assert_ne!(self.pixel_format.four_cc, *b"DX10");
		}
		Ok(())
	}
}

impl Default for Dds {
	fn default() -> Self {
		Self {
			flags: DDSD::DEFAULT,
			height: 0,
			width: 0,
			pitch: 0,
			depth: 0,
			mip_map_count: 1,
			reserved: Default::default(),
			pixel_format: PixelFormat::default(),
			caps: DDSCAPS::TEXTURE,
			reserved2: Default::default(),
			dx10: None,
		}
	}
}

#[derive(Debug, Clone)]
pub struct PixelFormat {
	/// See [`DDPF`] for values.
	pub flags: u32,
	pub four_cc: [u8; 4],
	pub rgb_bit_count: u32,
	pub r_bit_mask: u32,
	pub g_bit_mask: u32,
	pub b_bit_mask: u32,
	pub a_bit_mask: u32,
}

impl PixelFormat {
	fn read(f: &mut impl Read) -> Result<Self> {
		f.check_u32(32)?;
		Ok(PixelFormat {
			flags: f.u32()?,
			four_cc: f.array::<4>()?,
			rgb_bit_count: f.u32()?,
			r_bit_mask: f.u32()?,
			g_bit_mask: f.u32()?,
			b_bit_mask: f.u32()?,
			a_bit_mask: f.u32()?,
		})
	}

	fn write(&self, f: &mut impl Write) -> Result<()> {
		f.u32(32)?;
		f.u32(self.flags)?;
		f.write_all(&self.four_cc)?;
		f.u32(self.rgb_bit_count)?;
		f.u32(self.r_bit_mask)?;
		f.u32(self.g_bit_mask)?;
		f.u32(self.b_bit_mask)?;
		f.u32(self.a_bit_mask)?;
		Ok(())
	}
}

/// The default for a `PixelFormat` is a little-endian ARGB32 format.
impl Default for PixelFormat {
	fn default() -> Self {
		Self {
			flags: DDPF::ALPHAPIXELS | DDPF::RGB,
			four_cc: Default::default(),
			rgb_bit_count: 32,
			r_bit_mask: 0x00FF0000,
			g_bit_mask: 0x0000FF00,
			b_bit_mask: 0x000000FF,
			a_bit_mask: 0xFF000000,
		}
	}
}

#[derive(Debug, Clone)]
pub struct Dx10Header {
	/// See [`DXGI_FORMAT`] for values.
	pub dxgi_format: u32,
	/// See [`RESOURCE_DIMENSION`] for values.
	pub resource_dimension: u32,
	/// See [`RESOURCE_MISC`] for values.
	pub misc_flag: u32,
	pub array_size: u32,
	/// See [`ALPHA_MODE`] for values.
	pub misc_flag2: u32,
}

impl Dx10Header {
	fn read(f: &mut impl Read) -> Result<Self> {
		Ok(Dx10Header {
			dxgi_format: f.u32()?,
			resource_dimension: f.u32()?,
			misc_flag: f.u32()?,
			array_size: f.u32()?,
			misc_flag2: f.u32()?,
		})
	}

	fn write(&self, f: &mut impl Write) -> Result<()> {
		f.u32(self.dxgi_format)?;
		f.u32(self.resource_dimension)?;
		f.u32(self.misc_flag)?;
		f.u32(self.array_size)?;
		f.u32(self.misc_flag2)?;
		Ok(())
	}
}

impl Default for Dx10Header {
	fn default() -> Self {
		Self {
			dxgi_format: DXGI_FORMAT::B8G8R8A8_UNORM,
			resource_dimension: RESOURCE_DIMENSION::TEXTURE2D,
			misc_flag: 0,
			array_size: 1,
			misc_flag2: 0,
		}
	}
}

trait ReadData: Read {
	fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
		let mut buf = [0; N];
		self.read_exact(&mut buf)?;
		Ok(buf)
	}

	fn u32(&mut self) -> Result<u32> {
		self.array().map(u32::from_le_bytes)
	}

	fn u128(&mut self) -> Result<u128> {
		self.array().map(u128::from_le_bytes)
	}

	fn check_u32(&mut self, val: u32) -> Result<()> {
		let v = self.u32()?;
		if v == val {
			Ok(())
		} else {
			Err(Error::new(ErrorKind::InvalidData, format!("expected {val}, got {v}")))
		}
	}
}
impl<T: Read> ReadData for T {}

trait WriteData: Write {
	fn u32(&mut self, val: u32) -> Result<()> {
		self.write_all(&val.to_le_bytes())
	}

	fn u128(&mut self, val: u128) -> Result<()> {
		self.write_all(&val.to_le_bytes())
	}
}
impl<T: Write> WriteData for T {}
