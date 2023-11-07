#[derive(Clone, PartialEq, Eq)]
pub struct Raster<T> {
	width: usize,
	height: usize,
	data: Vec<T>,
}

impl<T> std::fmt::Debug for Raster<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		f.debug_struct("Raster")
			.field("width", &self.width)
			.field("height", &self.height)
			.finish_non_exhaustive()
	}
}

impl<T> Raster<T> {
	pub fn new(width: usize, height: usize) -> Self
	where
		T: Default,
	{
		let mut data = Vec::with_capacity(width * height);
		data.resize_with(width * height, Default::default);
		Self::new_with(width, height, data)
	}

	pub fn splat(width: usize, height: usize, val: T) -> Self
	where
		T: Clone,
	{
		let mut data = Vec::with_capacity(width * height);
		data.resize(width * height, val);
		Self::new_with(width, height, data)
	}

	pub fn new_with(width: usize, height: usize, data: Vec<T>) -> Self {
		assert_eq!(data.len(), width * height);
		Raster {
			width,
			height,
			data,
		}
	}

	pub fn width(&self) -> usize {
		self.width
	}

	pub fn height(&self) -> usize {
		self.height
	}

	pub fn as_slice(&self) -> &[T] {
		&self.data
	}

	pub fn map<U>(&self, f: impl FnMut(&T) -> U) -> Raster<U> {
		Raster::new_with(self.width, self.height, self.data.iter().map(f).collect())
	}
}

impl<T> std::ops::Index<[usize; 2]> for Raster<T> {
	type Output = T;

	fn index(&self, [x, y]: [usize; 2]) -> &T {
		&self.data[y * self.width + x]
	}
}

impl<T> std::ops::IndexMut<[usize; 2]> for Raster<T> {
	fn index_mut(&mut self, [x, y]: [usize; 2]) -> &mut T {
		&mut self.data[y * self.width + x]
	}
}
