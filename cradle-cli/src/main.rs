#![feature(lazy_cell)]
#![feature(array_chunks)]

use std::sync::LazyLock;

use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use clap::ValueHint;
use eyre_span::emit;

mod itp_png;
mod itp_dds;

#[derive(Debug, Clone, Parser)]
#[command(arg_required_else_help = true)]
struct Cli {
	/// Where to place resulting files (default is same directory as inputs)
	#[clap(long, short, value_hint = ValueHint::DirPath)]
	output: Option<Utf8PathBuf>,

	/// Convert images to dds instead of png
	#[clap(long)]
	dds: bool,

	/// Do not read or write indexed images from png files
	#[clap(long)]
	png_no_palette: bool,

	/// Read and write mipmaps as APNG frames
	///
	/// This is mostly for debugging purposes.
	#[clap(long)]
	png_mipmap: bool,

	/// The files to convert
	#[clap(value_hint = ValueHint::FilePath, required = true)]
	file: Vec<Utf8PathBuf>,
}

impl Cli {
	fn output(&self, path: &Utf8Path, ext: &str) -> eyre::Result<Utf8PathBuf> {
		let dir = if let Some(output) = self.output.as_ref() {
			if self.file.len() == 1 {
				if let Some(parent) = output.parent() {
					std::fs::create_dir_all(parent)?;
				}
				return Ok(output.clone())
			}

			std::fs::create_dir_all(output)?;
			output
		} else {
			path.parent().ok_or_else(|| eyre::eyre!("file has no parent"))?
		};
		let name = path.file_name().ok_or_else(|| eyre::eyre!("file has no name"))?;
		Ok(dir.join(name).with_extension(ext))
	}
}

static CLI: LazyLock<Cli> = LazyLock::new(Cli::parse);

fn main() -> eyre::Result<()> {
	init_tracing()?;
	LazyLock::force(&CLI);

	for file in &CLI.file {
		emit(process(file));
	}

	Ok(())
}

fn init_tracing() -> Result<(), eyre::Error> {
	use tracing_error::ErrorLayer;
	use tracing_subscriber::prelude::*;
	use tracing_subscriber::{fmt, EnvFilter};
	let fmt_layer = fmt::layer()
		.with_writer(std::io::stderr)
		.with_target(false);
	let filter_layer = EnvFilter::try_from_default_env()
		.or_else(|_| EnvFilter::try_new("info"))?;
	tracing_subscriber::registry()
		.with(filter_layer)
		.with(fmt_layer)
		.with(ErrorLayer::default())
		.init();
	eyre_span::install()?;
	Ok(())
}

#[tracing::instrument(skip_all, fields(path=%file))]
fn process(file: &Utf8Path) -> eyre::Result<()> {
	let ext = file.extension().unwrap_or("");
	match ext {
		"itp" => {
			let data = std::fs::read(file)?;
			let itp = tracing::info_span!("parse_itp").in_scope(|| {
				cradle::itp::read(&data).map_err(eyre::Report::from)
			})?;
			if CLI.dds {
				let output = CLI.output(file, "dds")?;
				let f = std::fs::File::create(&output)?;
				itp_dds::itp_to_dds(f, &itp)?;
				tracing::info!("wrote to {output}");
			} else {
				let output = CLI.output(file, "png")?;
				let f = std::fs::File::create(&output)?;
				itp_png::itp_to_png(f, &itp)?;
				tracing::info!("wrote to {output}");
			}
		}
		_ => eyre::bail!("unknown file extension"),
	}
	Ok(())
}
