#[inline]
pub fn bc1(block: u64) -> [u32; 16] {
	let c0 = block as u16;
	let c1 = (block >> 16) as u16;
	let bits = (block >> 32) as u32;
	let colors = if c0 > c1 {
		[
			lerp(c0, c1, 0, 3),
			lerp(c0, c1, 1, 3),
			lerp(c0, c1, 2, 3),
			lerp(c0, c1, 3, 3),
		]
	} else {
		[
			lerp(c0, c1, 0, 2),
			lerp(c0, c1, 1, 2),
			lerp(c0, c1, 2, 2),
			0x00000000,
		]
	};
	std::array::from_fn(|i| colors[(bits >> (i*2)) as usize & 3])
}

#[inline]
pub fn bc2(block: u128) -> [u32; 16] {
	let bits = block as u64;
	let bc1 = bc1((block >> 64) as u64);
	std::array::from_fn(|i| {
		let a = (bits >> (i*4)) & 0xF;
		bc1[i] & 0xFFFFFF | ((a * 0x11) as u32) << 24
	})
}

#[inline]
pub fn bc3(block: u128) -> [u32; 16] {
	let a0 = block as u8;
	let a1 = (block >> 8) as u8;
	let bits = block >> 16;
	let alpha = if a0 > a1 {
		[
			lerp1(a0, a1, 0, 7),
			lerp1(a0, a1, 7, 7),
			lerp1(a0, a1, 1, 7),
			lerp1(a0, a1, 2, 7),
			lerp1(a0, a1, 3, 7),
			lerp1(a0, a1, 4, 7),
			lerp1(a0, a1, 5, 7),
			lerp1(a0, a1, 6, 7),
		]
	} else {
		[
			lerp1(a0, a1, 0, 5),
			lerp1(a0, a1, 5, 5),
			lerp1(a0, a1, 1, 5),
			lerp1(a0, a1, 2, 5),
			lerp1(a0, a1, 3, 5),
			lerp1(a0, a1, 4, 5),
			0x00,
			0xFF,
		]
	};

	let bc1 = bc1((block >> 64) as u64);
	std::array::from_fn(|i| {
		let a = (bits >> (i*3)) & 7;
		bc1[i] & 0xFFFFFF | (alpha[a as usize] as u32) << 24
	})
}

#[inline(always)]
fn rgb565(block: u16) -> [u8; 4] {
	let r = ((block >> 11)& 0b00011111) as u8;
	let g = ((block >> 5) & 0b00111111) as u8;
	let b = (block        & 0b00011111) as u8;
	[
		b << 3 | b >> 2,
		g << 2 | g >> 4,
		r << 3 | r >> 2,
		0xFF
	]
}

#[inline(always)]
fn lerp(c0: u16, c1: u16, p: u16, q: u16) -> u32 {
	let c0 = rgb565(c0);
	let c1 = rgb565(c1);
	u32::from_le_bytes(std::array::from_fn(|i| lerp1(c0[i], c1[i], p, q)))
}

fn lerp1(a: u8, b: u8, p: u16, q: u16) -> u8 {
	(((q-p) * (a as u16) + p * (b as u16)) / q) as u8
}
