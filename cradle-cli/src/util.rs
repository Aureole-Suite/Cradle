use std::io::{self, Write};

use camino::{Utf8Path, Utf8PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Output {
	At(Utf8PathBuf),
	In(Utf8PathBuf),
}

impl Output {
	pub fn with_name(&self, name: &str) -> Utf8PathBuf {
		match self {
			Output::At(path) => path.to_path_buf(),
			Output::In(path) => path.with_file_name(name),
		}
	}

	pub fn with_extension(&self, ext: &str) -> Utf8PathBuf {
		match self {
			Output::At(path) => path.to_path_buf(),
			Output::In(path) => path.with_extension(ext),
		}
	}

	pub fn from_output_flag(
		output: Option<impl AsRef<Utf8Path>>,
		file: impl AsRef<Utf8Path>,
		n_inputs: usize,
	) -> eyre::Result<Self> {
		let output = output.as_ref().map(|a| a.as_ref());
		let file = file.as_ref();
		let dir = if let Some(output) = output {
			if n_inputs == 1 && !output.as_str().ends_with(std::path::is_separator) {
				if let Some(parent) = output.parent() {
					std::fs::create_dir_all(parent)?;
				}
				return Ok(Output::At(output.to_path_buf()));
			}

			std::fs::create_dir_all(output)?;
			output
		} else {
			file.parent()
				.ok_or_else(|| eyre::eyre!("file has no parent"))?
		};
		let name = file
			.file_name()
			.ok_or_else(|| eyre::eyre!("file has no name"))?;
		Ok(Output::In(dir.join(name)))
	}
}

pub struct MyFormatter {
	level: usize,
	indent_to: usize,
	has_value: bool,
}

impl MyFormatter {
	pub fn new(depth: usize) -> Self {
		Self {
			level: 0,
			indent_to: depth,
			has_value: false,
		}
	}

	fn indent<W: Write + ?Sized>(&self, wr: &mut W, max_level: usize) -> io::Result<()> {
		if self.level <= max_level {
			wr.write_all(b"\n")?;
			for _ in 0..self.level {
				wr.write_all(b"\t")?;
			}
		} else {
			wr.write_all(b" ")?;
		}
		Ok(())
	}

	fn write_begin<W: Write + ?Sized>(&mut self, w: &mut W, delim: &[u8]) -> io::Result<()> {
		self.level += 1;
		self.has_value = false;
		w.write_all(delim)
	}

	fn write_end<W: Write + ?Sized>(&mut self, w: &mut W, delim: &[u8]) -> io::Result<()> {
		self.level -= 1;
		if self.has_value {
			self.indent(w, self.indent_to - 1)?;
		}
		w.write_all(delim)?;
		if self.level == 0 {
			w.write_all(b"\n")?;
		}
		Ok(())
	}

	fn write_comma<W: Write + ?Sized>(&mut self, w: &mut W, first: bool) -> io::Result<()> {
		if !first {
			w.write_all(b",")?;
		}
		self.indent(w, self.indent_to)?;
		Ok(())
	}
}

impl serde_json::ser::Formatter for MyFormatter {
	#[inline]
	fn begin_array<W: Write + ?Sized>(&mut self, w: &mut W) -> io::Result<()> {
		self.write_begin(w, b"[")
	}

	#[inline]
	fn end_array<W: Write + ?Sized>(&mut self, w: &mut W) -> io::Result<()> {
		self.write_end(w, b"]")
	}

	#[inline]
	fn begin_array_value<W: Write + ?Sized>(&mut self, w: &mut W, first: bool) -> io::Result<()> {
		self.write_comma(w, first)
	}

	#[inline]
	fn end_array_value<W: Write + ?Sized>(&mut self, _w: &mut W) -> io::Result<()> {
		self.has_value = true;
		Ok(())
	}

	#[inline]
	fn begin_object<W: Write + ?Sized>(&mut self, w: &mut W) -> io::Result<()> {
		self.write_begin(w, b"{")
	}

	#[inline]
	fn end_object<W: Write + ?Sized>(&mut self, w: &mut W) -> io::Result<()> {
		self.write_end(w, b"}")
	}

	#[inline]
	fn begin_object_key<W: Write + ?Sized>(&mut self, w: &mut W, first: bool) -> io::Result<()> {
		self.write_comma(w, first)
	}

	#[inline]
	fn begin_object_value<W: Write + ?Sized>(&mut self, w: &mut W) -> io::Result<()> {
		w.write_all(b": ")
	}

	#[inline]
	fn end_object_value<W: Write + ?Sized>(&mut self, _w: &mut W) -> io::Result<()> {
		self.has_value = true;
		Ok(())
	}
}
