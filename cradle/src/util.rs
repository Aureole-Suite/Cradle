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

/// Swaps the inner two axes of a slice representing a 4D array.
///
/// The inverse of `swizzle(slice, a, b, c, d)` is `swizzle(slice, a, c, b, d)`.
///
/// # Panics
/// This function panics if the slice's length is not equal to `a * b * c * d`.
#[inline]
pub fn swizzle<T: Clone>(slice: &[T], a: usize, b: usize, c: usize, d: usize) -> Vec<T> {
	assert_eq!(slice.len(), a * b * c * d);
	iter_swizzle(a, b, c, d)
		.map(|a| slice[a].clone())
		.collect()
}

/// Same as [`swizzle`], but operates in place.
///
/// This function does not clone or drop any elements, so no `Clone` bound is needed.
///
/// # Panics
/// This function panics if the slice's length is not equal to `a * b * c * d`.
#[inline]
pub fn swizzle_mut<T>(slice: &mut [T], a: usize, b: usize, c: usize, d: usize) {
	assert_eq!(slice.len(), a * b * c * d);
	let mut scratch = Vec::<T>::with_capacity(slice.len());
	for (i, a) in iter_swizzle(a, b, c, d).enumerate() {
		// SAFETY: the values returned by iter_swizzle are always <a*b*c*d, which is in bounds
		unsafe {
			std::ptr::copy(
				slice.as_ptr().add(a),
				scratch.as_mut_ptr().add(i),
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
