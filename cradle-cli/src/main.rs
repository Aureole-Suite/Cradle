#![feature(array_chunks)]

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

	#[clap(flatten)]
	args: Args,

	/// The files to convert
	#[clap(value_hint = ValueHint::FilePath, required = true)]
	file: Vec<Utf8PathBuf>,
}

#[derive(Debug, Clone, Default, clap::Args)]
struct Args {
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

	/// Itp revision to write
	///
	/// Older revisions are more compatible, but cannot represent all pixel formats.
	///
	/// By default, will choose the oldest revision that can represent the pixel format, which means
	/// - revision 1 for indexed color,
	/// - revision 2 for 32-bit color and BC1/2/3 encoding,
	/// - revision 3 for BC7-encoded images.
	#[clap(long, value_parser = 1..=3, verbatim_doc_comment)]
	itp_revision: Option<u16>,
}

impl Cli {
	fn output(&self, path: &Utf8Path, ext: &str) -> eyre::Result<Utf8PathBuf> {
		let dir = if let Some(output) = self.output.as_ref() {
			if self.file.len() == 1 && !output.as_str().ends_with(std::path::is_separator) {
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

fn main() -> eyre::Result<()> {
	init_tracing()?;
	let cli = Cli::parse();

	for file in &cli.file {
		emit(process(&cli, file));
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
fn process(cli: &Cli, file: &Utf8Path) -> eyre::Result<()> {
	let ext = file.extension().unwrap_or("");
	let args = &cli.args;
	match ext {
		"itp" => {
			let data = std::fs::read(file)?;
			let itp = tracing::info_span!("parse_itp").in_scope(|| {
				cradle::itp::read(&data).map_err(eyre::Report::from)
			})?;
			if args.dds {
				let output = cli.output(file, "dds")?;
				let f = std::fs::File::create(&output)?;
				itp_dds::itp_to_dds(args, f, &itp)?;
				tracing::info!("wrote to {output}");
			} else {
				let output = cli.output(file, "png")?;
				let f = std::fs::File::create(&output)?;
				itp_png::itp_to_png(args, f, &itp)?;
				tracing::info!("wrote to {output}");
			}
		}
		"dds" => {
			let data = std::fs::File::open(file)?;
			let mut itp = tracing::info_span!("parse_dds").in_scope(|| {
				itp_dds::dds_to_itp(args, &data).map_err(eyre::Report::from)
			})?;
			guess_itp_revision(args, &mut itp);
			let output = cli.output(file, "itp")?;
			std::fs::write(&output, cradle::itp::write(&itp)?)?;
			tracing::info!("wrote to {output}");
		}
		"png" => {
			let data = std::fs::File::open(file)?;
			let mut itp = tracing::info_span!("parse_png").in_scope(|| {
				itp_png::png_to_itp(args, &data).map_err(eyre::Report::from)
			})?;
			guess_itp_revision(args, &mut itp);
			let output = cli.output(file, "itp")?;
			std::fs::write(&output, cradle::itp::write(&itp)?)?;
			tracing::info!("wrote to {output}");
		}
		_ => eyre::bail!("unknown file extension"),
	}
	Ok(())
}

fn guess_itp_revision(args: &Args, itp: &mut cradle::itp::Itp) {
	use cradle::itp::ItpRevision as IR;
	itp.status.itp_revision = match args.itp_revision {
		Some(1) => IR::V1,
		Some(2) => IR::V2,
		Some(3) => IR::V3,
		Some(_) => unreachable!(),
		None => match &itp.data {
			cradle::itp::ImageData::Indexed(_, _) => IR::V1,
			cradle::itp::ImageData::Argb16(_, _) => unimplemented!(),
			cradle::itp::ImageData::Argb32(_) => IR::V2,
			cradle::itp::ImageData::Bc1(_) => IR::V2,
			cradle::itp::ImageData::Bc2(_) => IR::V2,
			cradle::itp::ImageData::Bc3(_) => IR::V2,
			cradle::itp::ImageData::Bc7(_) => IR::V3,
		}
	}
}
