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
