use camino::Utf8PathBuf;
use cradle::itp::{ImageData, Itp, Palette};
use strict_result::Strict;

use crate::{util::Output, Args};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct FrameSpec {
	frame: usize,
	path: Utf8PathBuf,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	offset: Option<(f32, f32)>,
	#[serde(default = "unit_scale", skip_serializing_if = "is_unit_scale")]
	scale: (f32, f32),
}

fn unit_scale() -> (f32, f32) {
	(1.0, 1.0)
}
fn is_unit_scale(a: &(f32, f32)) -> bool {
	*a == unit_scale()
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ItcSpec {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	palette: Option<Vec<u32>>,
	frames: Vec<FrameSpec>,
}

pub fn extract(args: &Args, itc: &cradle::itc::Itc, output: Output) -> eyre::Result<Utf8PathBuf> {
	let outdir = output.with_extension("");
	let json_out = if args.no_dir {
		output.with_extension("itc.json")
	} else {
		std::fs::create_dir_all(&outdir)?;
		outdir.join("cradle.itc.json")
	};

	let mut frames = Vec::new();
	for (i, frame) in itc.frames.iter().enumerate() {
		let Some(itp) = &frame.itp else { continue };

		let _span = tracing::info_span!("frame", i = i).entered();

		let (w, h) = cradle::itp::read_size(itp)?;

		let frame_out = if args.no_dir {
			output.with_extension(&format!("{i}.itp"))
		} else {
			output.with_extension("").join(&format!("{i}.itp"))
		};

		let frame_out = if args.itp {
			std::fs::write(&frame_out, itp)?;
			frame_out
		} else {
			let mut itp = tracing::info_span!("parse_itp")
				.in_scope(|| Ok(cradle::itp::read(itp)?))
				.strict()?;

			if let Itp {
				data: ImageData::Indexed(pal @ Palette::External(..), _),
				..
			} = &mut itp
			{
				if let Some(palette) = &itc.palette {
					tracing::warn!("inlining palette");
					*pal = Palette::Embedded(palette.clone())
				} else {
					eyre::bail!("no palette")
				}
			}

			crate::from_itp(args, &itp, Output::In(frame_out))?
		};

		// Not sure if this is the right formula? Previous Cradle use different
		let xs = frame.scale.0;
		let ys = frame.scale.1;
		let xo = frame.offset.0 * w as f32;
		let yo = frame.offset.1 * h as f32;

		frames.push((
			frame.order,
			FrameSpec {
				frame: i,
				path: frame_out.strip_prefix(&outdir).unwrap().to_path_buf(),
				offset: Some((xo, yo)),
				scale: (xs, ys),
			},
		));
	}

	frames.sort_by_key(|a| a.0);
	crate::Spec::write(
		&json_out,
		crate::util::MyFormatter::new(2),
		ItcSpec {
			palette: itc.palette.as_ref().filter(|_| args.itp).cloned(),
			frames: frames.into_iter().map(|a| a.1).collect(),
		},
	)?;

	Ok(json_out)
}

pub fn create(args: &Args, spec: ItcSpec, output: Output) -> eyre::Result<Utf8PathBuf> {
	println!("{:?}", spec);
	eyre::bail!("foo")
}
