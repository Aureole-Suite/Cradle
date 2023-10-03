use std::io::Write;

use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use clap::ValueHint;
use cradle::itp::ImageData;
use cradle::itp::Itp;
use eyre_span::emit;

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
	match ext {
		"itp" => {
			let data = std::fs::read(file)?;
			let itp = tracing::info_span!("parse_itp").in_scope(|| {
				cradle::itp::read(&data).map_err(eyre::Report::from)
			})?;
			if cli.dds {
				let output = cli.output(file, "dds")?;
				std::fs::write(&output, cradle_dds::to_dds(&itp))?;
				tracing::info!("wrote to {output}");
			} else {
				let output = cli.output(file, "png")?;
				let f = std::fs::File::create(&output)?;
				itp_to_png(cli, f, &itp)?;
				tracing::info!("wrote to {output}");
			}
		}
		_ => eyre::bail!("unknown file extension"),
	}
	Ok(())
}

fn itp_to_png(cli: &Cli, f: impl Write, itp: &Itp) -> eyre::Result<()> {
	let Itp { status: _, width, height, ref data } = *itp;
	match data {
		ImageData::Indexed(pal, data) => {
			let pal = match pal {
				cradle::itp::Palette::Embedded(pal) => pal,
				cradle::itp::Palette::External(_) => eyre::bail!("external palette is not currently supported"),
			};
			if cli.png_no_palette {
				let data = data.iter().map(|a| pal[*a as usize]).collect::<Vec<_>>();
				write_png(f, width, height, &data)?;
			} else {
				write_indexed_png(f, width, height,pal, data)?;
			}
		},
		ImageData::Argb16_1(_) => eyre::bail!("16-bit color is not currently supported"),
		ImageData::Argb16_2(_) => eyre::bail!("16-bit color is not currently supported"),
		ImageData::Argb16_3(_) => eyre::bail!("16-bit color is not currently supported"),
		ImageData::Argb32(data) => write_png(f, width, height, data)?,
		ImageData::Bc1(data) => bc_to_png(f, width, height, data, cradle_dxt::decode_bc1)?,
		ImageData::Bc2(data) => bc_to_png(f, width, height, data, cradle_dxt::decode_bc2)?,
		ImageData::Bc3(data) => bc_to_png(f, width, height, data, cradle_dxt::decode_bc3)?,
		ImageData::Bc7(data) => bc_to_png(f, width, height, data, cradle_dxt::decode_bc7)?,
	}
	Ok(())
}

fn bc_to_png<T: Copy>(
	w: impl Write,
	width: u32,
	height: u32,
	data: &[T],
	f: impl FnMut(T) -> [u32; 16],
) -> eyre::Result<()> {
	let mut data = data.iter().copied().flat_map(f).collect::<Vec<_>>();
	let len = (width * height) as usize;
	cradle::permute::unswizzle(&mut data[..len], height as usize, width as usize, 4, 4);
	write_png(w, width, height, &data)
}

fn write_png(
	mut w: impl Write,
	width: u32,
	height: u32,
	data: &[u32],
) -> eyre::Result<()> {
	let len = (width * height) as usize;
	if data.len() > len {
		tracing::warn!("discarding mipmaps")
	}
	let data = &data[..len];

	let mut png = png::Encoder::new(&mut w, width, height);
	png.set_color(png::ColorType::Rgba);
	png.set_depth(png::BitDepth::Eight);
	let mut w = png.write_header()?;
	let data: Vec<u8> = data.iter()
		.flat_map(|argb| {
			let [b, g, r, a] = argb.to_le_bytes();
			[r, g, b, a]
		})
		.collect::<Vec<_>>();
	w.write_image_data(&data)?;
	w.finish()?;
	Ok(())
}

fn write_indexed_png(
	mut w: impl Write,
	width: u32,
	height: u32,
	palette: &[u32],
	data: &[u8],
) -> eyre::Result<()> {
	let len = (width * height) as usize;
	if data.len() > len {
		tracing::warn!("discarding mipmaps")
	}
	let data = &data[..len];

	let mut png = png::Encoder::new(&mut w, width, height);
	let mut pal = Vec::with_capacity(3*palette.len());
	let mut alp = Vec::with_capacity(palette.len());
	for rgba in palette {
		let [r, g, b, a] = u32::to_le_bytes(*rgba);
		pal.push(r);
		pal.push(g);
		pal.push(b);
		alp.push(a);
	}
	png.set_color(png::ColorType::Indexed);
	png.set_depth(png::BitDepth::Eight);
	png.set_palette(pal);
	png.set_trns(alp);
	let mut w = png.write_header()?;
	w.write_image_data(data)?;
	w.finish()?;
	Ok(())
}
