use camino::{Utf8Path, Utf8PathBuf};
use cradle::itp::{ImageData, Palette};
use strict_result::Strict;

use crate::{png, util::Output, Args};

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

	let mut maxw = 0;
	let mut maxh = 0;
	if !args.itp && !args.dds && !args.itc_no_pad {
		let _span = tracing::info_span!("calcsize").entered();
		for (i, frame) in itc.frames.iter().enumerate() {
			let Some(itp) = &frame.itp else { continue };
			let _span = tracing::info_span!("frame", i = i).entered();
			let (w, h) = cradle::itp::read_size(itp)?;
			let xo = frame.offset.0 * w as f32;
			let yo = frame.offset.1 * h as f32;
			let w = w + xo.abs().round() as u32 * 2;
			let h = h + yo.abs().round() as u32 * 2;
			maxw = maxw.max(w.next_power_of_two());
			maxh = maxh.max(h.next_power_of_two());
		}
	}

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

		// Not sure if this is the right formula? Previous Cradle use different
		let xs = frame.scale.0;
		let ys = frame.scale.1;
		let xo = frame.offset.0 * w as f32;
		let yo = frame.offset.1 * h as f32;
		let mut offset = Some((xo, yo));

		let frame_out = if args.itp {
			std::fs::write(&frame_out, itp)?;
			frame_out
		} else {
			let mut itp = tracing::info_span!("parse_itp")
				.in_scope(|| Ok(cradle::itp::read(itp)?))
				.strict()?;

			if let ImageData::Indexed(pal @ Palette::External(..), _) = &mut itp.data {
				if let Some(palette) = &itc.palette {
					tracing::warn!("inlining palette");
					*pal = Palette::Embedded(palette.clone())
				} else {
					eyre::bail!("no palette")
				}
			}

			if args.dds {
				let output = frame_out.with_extension("dds");
				let f = std::fs::File::create(&output)?;
				crate::itp_dds::itp_to_dds(args, f, &itp)?;
				output
			} else {
				let output = frame_out.with_extension("png");
				let f = std::fs::File::create(&output)?;
				let mut png = crate::itp_png::itp_to_png(args, &itp)?;
				if !args.itc_no_pad {
					let _span = tracing::info_span!("pad").entered();
					if (xo - xo.round()).abs() < f32::EPSILON && (yo - yo.round()).abs() < f32::EPSILON {
						png = pad(png, -xo.round() as i32, -yo.round() as i32, maxw, maxh);
						offset = None;
					}
				}
				png::write(args, f, &png)?;
				output
			}
		};

		frames.push((
			frame.order,
			FrameSpec {
				frame: i,
				path: frame_out.strip_prefix(&outdir).unwrap().to_path_buf(),
				offset,
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

	if args.no_dir {
		Ok(json_out)
	} else {
		Ok(outdir)
	}
}

fn pad(png: png::Png, x: i32, y: i32, width: u32, height: u32) -> png::Png {
	let x = u32::checked_add_signed((width - png.width) / 2, x).unwrap();
	let y = u32::checked_add_signed((height - png.height) / 2, y).unwrap();
	let offset = (y * width + x) as usize;
	let mut data = png.data;
	match &mut data {
		png::ImageData::Argb32(data) => {
			let mut out = vec![data[0]; height as usize * width as usize];
			copy_rect(data, &mut out[offset..], png.width as usize, width as usize);
			*data = out;
		}
		png::ImageData::Indexed(_, data) => {
			let mut out = vec![data[0]; height as usize * width as usize];
			copy_rect(data, &mut out[offset..], png.width as usize, width as usize);
			*data = out;
		}
	}
	png::Png {
		width,
		height,
		data,
	}
}

fn copy_rect<T: Clone>(src: &[T], dst: &mut [T], src_width: usize, dst_width: usize) {
	use std::iter::zip;
	for (src, dst) in zip(src.chunks_exact(src_width), dst.chunks_exact_mut(dst_width)) {
		for (src, dst) in zip(src.iter(), dst.iter_mut()) {
			*dst = src.clone()
		}
	}
}

pub fn create(args: &Args, spec: ItcSpec, dir: &Utf8Path) -> eyre::Result<cradle::itc::Itc> {
	let mut itc = cradle::itc::Itc {
		palette: spec.palette,
		..Default::default()
	};
	for (order, spec) in spec.frames.iter().enumerate() {
		let _span = tracing::info_span!("frame", i = spec.frame).entered();
		let Some(frame) = itc.frames.get_mut(spec.frame) else {
			eyre::bail!("invalid frame number");
		};
		if frame.itp.is_some() {
			eyre::bail!("duplicate frame number");
		}
		let itp = crate::to_itp(args, &dir.join(&spec.path))?;
		let (w, h) = cradle::itp::read_size(&itp)?;

		let offset = if let Some((x, y)) = spec.offset {
			(x / w as f32, y / h as f32)
		} else {
			(0., 0.)
		};

		*frame = cradle::itc::Frame {
			itp: Some(itp),
			unknown: 0,
			offset,
			scale: spec.scale,
			order,
		};
	}
	Ok(itc)
}

#[cfg(test)]
#[filetest::filetest("../../samples/itc/*.itc")]
fn test_itp_roundtrips(path: &Utf8Path, bytes: &[u8]) -> Result<(), eyre::Error> {
	let tmpdir = camino_tempfile::Builder::new()
		.prefix("cradle-")
		.suffix(&format!("-{}", path.file_stem().unwrap()))
		.tempdir()?;
	let args = &Args {
		itp: true,
		..Args::default()
	};

	let itc = cradle::itc::read(bytes)?;
	extract(args, &itc, Output::At(tmpdir.path().to_path_buf()))?;
	let file = std::fs::File::open(tmpdir.path().join("cradle.itc.json"))?;
	let itc2 = create(args, serde_json::from_reader(file)?, tmpdir.path())?;
	assert_eq!(itc, itc2);
	let bytes2 = cradle::itc::write(&itc2)?;
	assert_eq!(bytes, bytes2);
	Ok(())
}
