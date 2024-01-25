#![feature(array_chunks)]

use camino::{Utf8Path, Utf8PathBuf};
use clap::{builder::TypedValueParser, Parser, ValueHint};
use eyre::ContextCompat as _;
use eyre_span::emit;
use strict_result::*;

mod ch;
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

	/// Do not pad/crop the frames to equal size
	///
	/// Only supported in png; --itp and --dds invalidate this.
	#[clap(long)]
	itc_no_pad: bool,

	/// Override the width for parsing ._ch files, instead of guessing from name.
	///
	/// When writing, the width of the source is used regardless of this.
	#[clap(long)]
	ch_width: Option<usize>,

	/// Override the format for parsing and writing ._ch files, instead of guessing from name.
	#[clap(
		long,
		value_parser = clap::builder::PossibleValuesParser::new(["1555", "4444", "8888"])
			.map(|v| match v.as_str() {
				"1555" => cradle::ch::Mode::Argb1555,
				"4444" => cradle::ch::Mode::Argb4444,
				"8888" => cradle::ch::Mode::Argb8888,
				_ => unreachable!(),
			}),
	)]
	ch_mode: Option<cradle::ch::Mode>,
}

impl Cli {
	fn output<'a>(&'a self, path: &'a Utf8Path) -> eyre::Result<util::Output> {
		util::Output::from_output_flag(self.output.as_deref(), path, self.file.len())
	}
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, derive_more::From)]
#[serde(tag = "type", rename_all = "snake_case")]
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

#[tracing::instrument(skip_all, fields(path=%raw_file))]
fn process(cli: &Cli, raw_file: &Utf8Path) -> eyre::Result<()> {
	let file = &effective_input_file(raw_file)?;
	if file != raw_file {
		tracing::info!("using {file}");
	}
	let ext = file.extension().unwrap_or("");
	let output = cli.output(file)?;
	let args = &cli.args;
	match ext {
		"itp" => {
			let data = std::fs::read(file)?;
			let itp = tracing::info_span!("parse_itp")
				.in_scope(|| Ok(cradle::itp::read(&data)?))
				.strict()?;
			let output = if args.dds {
				let output = output.with_extension("dds");
				let f = std::fs::File::create(&output)?;
				itp_dds::itp_to_dds(args, f, &itp)?;
				output
			} else {
				let output = output.with_extension("png");
				let f = std::fs::File::create(&output)?;
				let png = itp_png::itp_to_png(args, &itp)?;
				png::write(f, &png)?;
				output
			};
			tracing::info!("wrote to {output}");
		}

		"_ch" => {
			let data = std::fs::read(file)?;
			let name = file.file_name().unwrap();
			let guess = cradle::ch::guess_from_byte_size(name, data.len());
			let mode = args
				.ch_mode
				.or(guess.map(|a| a.0))
				.context("could not guess format")?;
			let width = args
				.ch_width
				.or(guess.map(|a| a.1))
				.context("could not guess format")?;

			let ch = tracing::info_span!("parse_ch")
				.in_scope(|| Ok(cradle::ch::read(mode, width, &data)?))
				.strict()?;
			if args.dds {
				let output = output.with_extension("ch.dds");
				let f = std::fs::File::create(&output)?;
				ch::ch_to_dds(args, f, &ch)?;
				tracing::info!("wrote to {output}");
			} else {
				let output = output.with_extension("ch.png");
				let f = std::fs::File::create(&output)?;
				let png = ch::ch_to_png(args, &ch)?;
				png::write(f, &png)?;
				tracing::info!("wrote to {output}");
			};
		}

		"png" | "dds" if file.with_extension("").extension() == Some("ch") => {
			let output = output.with_extension("").with_extension("_ch");
			let data = std::fs::File::open(file)?;
			let itp = if ext == "png" {
				tracing::info_span!("parse_png")
					.in_scope(|| Ok(itp_png::png_to_itp(args, &png::read(&data).strict()?)))?
			} else {
				tracing::info_span!("parse_dds").in_scope(|| itp_dds::dds_to_itp(args, &data))?
			};
			let name = file.file_name().unwrap();
			let guess =
				cradle::ch::guess_from_image_size(name, itp.data.width(), itp.data.height());
			let mode = args.ch_mode.or(guess).context("could not guess format")?;
			let ch = crate::ch::itp_to_ch(args, mode, &itp)?;
			std::fs::write(&output, cradle::ch::write(&ch)?)?;
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

		"json" => {
			let output = if file == raw_file {
				// to strip off the duplicate .ext.json suffix
				cli.output(&raw_file.with_extension(""))?
			} else {
				// but if it's a dir, there's no such suffix
				cli.output(raw_file)?
			};
			let spec = tracing::info_span!("parse_json")
				.in_scope(|| Ok(serde_json::from_reader(std::fs::File::open(file)?)?))
				.strict()?;
			let output = match spec {
				Spec::Itc(spec) => {
					let itc = itc::create(args, spec, file.parent().unwrap())?;
					let output = output.with_extension("itc");
					std::fs::write(&output, cradle::itc::write(&itc)?)?;
					output
				}
			};
			tracing::info!("wrote to {output}");
		}

		_ => eyre::bail!("unknown file extension"),
	}
	Ok(())
}

fn effective_input_file(file: &Utf8Path) -> eyre::Result<Utf8PathBuf> {
	if file.is_dir() {
		let files = file.read_dir_utf8()?.collect::<Result<Vec<_>, _>>()?;
		let files = files
			.iter()
			.map(|f| f.path())
			.filter(|f| f.is_file())
			.filter(|f| f.extension() == Some("json"))
			.collect::<Vec<_>>();
		match files.as_slice() {
			[] => eyre::bail!("no json file in directory"),
			[a] => Ok(a.to_path_buf()),
			_ => eyre::bail!("multiple json files in directory"),
		}
	} else if file.exists() {
		Ok(file.to_path_buf())
	} else {
		eyre::bail!("file doesn't exist")
	}
}

fn to_itp(args: &Args, path: &Utf8Path) -> eyre::Result<Vec<u8>> {
	let data = match path.extension() {
		Some("png") => {
			let data = std::fs::File::open(path)?;
			let mut itp = tracing::info_span!("parse_png")
				.in_scope(|| Ok(itp_png::png_to_itp(args, &png::read(&data).strict()?)))?;
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
