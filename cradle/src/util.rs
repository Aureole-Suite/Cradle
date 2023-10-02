#![deny(unsafe_op_in_unsafe_fn)]

/// Yields the indices needed for a [`swizzle`] operation.
///
/// This is a permutation of the range `0 .. a*b*c*d`.
#[inline(always)]
pub fn iter_swizzle(a: usize, b: usize, c: usize, d: usize) -> impl Iterator<Item=usize> {
	std::iter::once(0)
		.flat_map(move |x| (0..a).map(move |y| x + y*b*c*d))
		.flat_map(move |x| (0..b).map(move |y| x + y*d))
		.flat_map(move |x| (0..c).map(move |y| x + y*b*d))
		.flat_map(move |x| (0..d).map(move |y| x + y))
}

#[inline(always)]
pub fn iter_morton(h: usize, w: usize) -> impl Iterator<Item=usize> {
	(0..w*h).map(move |a| {
		let mut x = 0;
		let mut y = 0;
		for b in (0..usize::BITS/2).rev() {
			x |= usize::from(a & (2<<(2*b)) != 0) << b;
			y |= usize::from(a & (1<<(2*b)) != 0) << b;
		}
		y*w+x
	})
}

/// # Safety
/// This function requires that `permutation` is precisely a permutation of the range `0..slice.len()`.
pub unsafe fn permute_mut<T>(slice: &mut [T], permutation: impl Iterator<Item=usize>) {
	// SAFETY: Since `permutation` is a permutation, `.enumerate()` is too
	unsafe {
		apply_permutation(slice, permutation.enumerate())
	}
}


/// # Safety
/// This function requires that `permutation` is precisely a permutation of the range `0..slice.len()`.
pub unsafe fn unpermute_mut<T>(slice: &mut [T], permutation: impl Iterator<Item=usize>) {
	// SAFETY: Since `permutation` is a permutation, `.enumerate()` is too, and so is swapping the pairs
	unsafe {
		apply_permutation(slice, permutation.enumerate().map(|(a, b)| (b, a)))
	}
}

/// # Safety
/// This function requires that both sides of `permutation` is precisely a permutation of the range `0..slice.len()`.
pub unsafe fn apply_permutation<T>(slice: &mut [T], permutation: impl Iterator<Item=(usize, usize)>) {
	let mut scratch = Vec::<T>::with_capacity(slice.len());
	for (to, from) in permutation {
		// SAFETY: the values returned by permutation are always <slice.len()
		unsafe {
			std::ptr::copy(
				slice.as_ptr().add(from),
				scratch.as_mut_ptr().add(to),
				1,
			);
		}
	}
	// SAFETY: scratch is a permutation of slice, so every value exists exactly once in it
	unsafe {
		std::ptr::copy(
			scratch.as_ptr(),
			slice.as_mut_ptr(),
			slice.len(),
		);
	}
}

#[inline]
pub fn swizzle_mut<T>(slice: &mut [T], h: usize, w: usize, ch: usize, cw: usize) {
	assert_eq!(slice.len(), w * h);
	assert_eq!(w % cw, 0);
	assert_eq!(h % h, 0);
	// SAFETY: iter_swizzle is a permutation
	unsafe {
		permute_mut(slice, iter_swizzle(h/ch, w/cw, ch, cw));
	}
}

#[inline]
pub fn unswizzle_mut<T>(slice: &mut [T], h: usize, w: usize, ch: usize, cw: usize) {
	assert_eq!(slice.len(), w * h);
	assert_eq!(w % cw, 0);
	assert_eq!(h % h, 0);
	// SAFETY: iter_swizzle is a permutation
	unsafe {
		unpermute_mut(slice, iter_swizzle(h/ch, w/cw, ch, cw));
	}
}

#[inline]
pub fn morton_mut<T>(slice: &mut [T], height: usize, width: usize) {
	assert_eq!(slice.len(), width * height);
	// SAFETY: iter_morton is a permutation
	unsafe {
		permute_mut(slice, iter_morton(height, width));
	}
}

#[inline]
pub fn unmorton_mut<T>(slice: &mut [T], height: usize, width: usize) {
	assert_eq!(slice.len(), width * height);
	// SAFETY: iter_morton is a permutation
	unsafe {
		unpermute_mut(slice, iter_morton(height, width));
	}
}
