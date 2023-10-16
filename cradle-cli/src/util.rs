use camino::{Utf8PathBuf, Utf8Path};

pub enum Output<'a> {
	At { path: &'a Utf8Path },
	In { dir: &'a Utf8Path, name: &'a str },
}

impl<'a> Output<'a> {
	pub fn with_name(&self, name: &str) -> Utf8PathBuf {
		match self {
			Output::At { path } => path.to_path_buf(),
			Output::In { dir, name: _ } => dir.join(name),
		}
	}

	pub fn with_extension(&self, ext: &str) -> Utf8PathBuf {
		match self {
			Output::At { path } => path.to_path_buf(),
			Output::In { dir, name } => dir.join(name).with_extension(ext),
		}
	}

	pub fn from_output_flag(output: Option<&'a Utf8Path>, file: &'a Utf8Path, n_inputs: usize) -> eyre::Result<Self> {
		let dir = if let Some(output) = output {
			if n_inputs == 1 && !output.as_str().ends_with(std::path::is_separator) {
				if let Some(parent) = output.parent() {
					std::fs::create_dir_all(parent)?;
				}
				return Ok(Output::At { path: output })
			}

			std::fs::create_dir_all(output)?;
			output
		} else {
			file.parent().ok_or_else(|| eyre::eyre!("file has no parent"))?
		};
		let name = file.file_name().ok_or_else(|| eyre::eyre!("file has no name"))?;
		Ok(Output::In { dir, name })
	}
}
