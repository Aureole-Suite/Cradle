#![feature(array_chunks)]

use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use clap::ValueHint;
use eyre_span::emit;
use strict_result::*;

mod itc;
mod itp_dds;
mod itp_png;
mod png;
mod util;

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

	/// When extracting itc, do not convert the individual images
	#[clap(long)]
	itp: bool,

	/// When extracting itc, do not create a subdirectory
	///
	/// Normally the converted files will be placed at ch00000/index.json and ch00000/0.png,
	/// with this flag they are instead placed at ch00000.json and ch00000.0.png.
	#[clap(long)]
	no_dir: bool,

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
	fn output<'a>(&'a self, path: &'a Utf8Path) -> eyre::Result<util::Output> {
		util::Output::from_output_flag(self.output.as_deref(), path, self.file.len())
	}
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, derive_more::From)]
#[serde(tag = "type")]
enum Spec {
	Itc(itc::ItcSpec),
}

impl Spec {
	fn write(
		path: impl AsRef<Utf8Path>,
		formatter: impl serde_json::ser::Formatter,
		data: impl Into<Spec>,
	) -> eyre::Result<()> {
		use serde::Serialize;
		let mut ser = serde_json::Serializer::with_formatter(
			std::fs::File::create(path.as_ref())?,
			formatter,
		);
		data.into().serialize(&mut ser)?;
		Ok(())
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
	let fmt_layer = fmt::layer().with_writer(std::io::stderr).with_target(false);
	let filter_layer = EnvFilter::try_from_default_env().or_else(|_| EnvFilter::try_new("info"))?;
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
	let output = cli.output(file)?;
	let args = &cli.args;
	match ext {
		"itp" => {
			let data = std::fs::read(file)?;
			let output = from_itp(args, &data, output)?;
			tracing::info!("wrote to {output}");
		}
		"dds" | "png" => {
			let data = to_itp(args, file)?;
			let output = output.with_extension("itp");
			std::fs::write(&output, data)?;
			tracing::info!("wrote to {output}");
		}

		"itc" => {
			let data = std::fs::read(file)?;
			let itc = tracing::info_span!("parse_itc")
				.in_scope(|| Ok(cradle::itc::read(&data)?))
				.strict()?;
			let output = crate::itc::extract(args, &itc, output)?;
			tracing::info!("wrote to {output}");
		}

		_ => eyre::bail!("unknown file extension"),
	}
	Ok(())
}

fn from_itp(args: &Args, itp_bytes: &[u8], output: util::Output) -> eyre::Result<Utf8PathBuf> {
	let itp = tracing::info_span!("parse_itp")
		.in_scope(|| Ok(cradle::itp::read(itp_bytes)?))
		.strict()?;
	if args.dds {
		let output = output.with_extension("dds");
		let f = std::fs::File::create(&output)?;
		itp_dds::itp_to_dds(args, f, &itp)?;
		Ok(output)
	} else {
		let output = output.with_extension("png");
		let f = std::fs::File::create(&output)?;
		let png = itp_png::itp_to_png(args, &itp)?;
		png::write(args, f, &png)?;
		Ok(output)
	}
}

fn to_itp(args: &Args, path: &Utf8Path) -> eyre::Result<Vec<u8>> {
	let data = match path.extension() {
		Some("png") => {
			let data = std::fs::File::open(path)?;
			let mut itp = tracing::info_span!("parse_png")
				.in_scope(|| Ok(itp_png::png_to_itp(args, &png::read(args, &data)?)))
				.strict()?;
			guess_itp_revision(args, &mut itp);
			cradle::itp::write(&itp)?
		}

		Some("dds") => {
			let data = std::fs::File::open(path)?;
			let mut itp =
				tracing::info_span!("parse_dds").in_scope(|| itp_dds::dds_to_itp(args, &data))?;
			guess_itp_revision(args, &mut itp);
			cradle::itp::write(&itp)?
		}

		Some("itp") => std::fs::read(path)?,

		_ => eyre::bail!("unknown file extension"),
	};
	Ok(data)
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
		},
	}
}
