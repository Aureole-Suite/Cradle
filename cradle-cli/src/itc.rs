use camino::{Utf8Path, Utf8PathBuf};
use cradle::{
	itp::{ImageData, Palette},
	raster::Raster,
};
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
			let w = w + xo.abs().round() as usize * 2;
			let h = h + yo.abs().round() as usize * 2;
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
			output.with_extension("").join(format!("{i}.itp"))
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
					if (xo - xo.round()).abs() < 0.0001 && (yo - yo.round()).abs() < 0.0001 {
						pad(
							&mut png,
							xo.round() as isize,
							yo.round() as isize,
							maxw,
							maxh,
						);
						offset = None;
					}
				}
				png::write(f, &png)?;
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

pub fn create(args: &Args, spec: ItcSpec, dir: &Utf8Path) -> eyre::Result<cradle::itc::Itc> {
	let mut itc = cradle::itc::Itc {
		palette: spec.palette,
		..Default::default()
	};
	let mut frames = spec.frames.iter().enumerate().collect::<Vec<_>>();
	frames.sort_by_key(|a| a.1.frame);
	for (order, spec) in frames {
		let _span = tracing::info_span!("frame", i = spec.frame).entered();
		let Some(frame) = itc.frames.get_mut(spec.frame) else {
			eyre::bail!("invalid frame number");
		};
		if frame.itp.is_some() {
			eyre::bail!("duplicate frame number");
		}

		let path = dir.join(&spec.path);
		let (itp_data, offset) =
			if spec.offset.is_none() && path.extension() == Some("png") && !args.itc_no_pad {
				let data = std::fs::File::open(path)?;
				let _span = tracing::info_span!("parse_png").entered();
				let mut png = png::read(&data)?;
				let offset = crop(&mut png);
				let mut itp = crate::itp_png::png_to_itp(args, &png);
				drop(_span);
				crate::guess_itp_revision(args, &mut itp);
				let offset = (offset.0 as f32, offset.1 as f32);
				(cradle::itp::write(&itp)?, offset)
			} else {
				let offset = spec.offset.unwrap_or_default();
				(to_itp(args, &path)?, offset)
			};

		let (w, h) = cradle::itp::read_size(&itp_data)?;
		let offset = (offset.0 / w as f32, offset.1 / h as f32);

		*frame = cradle::itc::Frame {
			itp: Some(itp_data),
			unknown: 0,
			offset,
			scale: spec.scale,
			order,
		};
	}
	Ok(itc)
}

fn to_itp(args: &Args, path: &Utf8Path) -> eyre::Result<Vec<u8>> {
	let data = match path.extension() {
		Some("png") => {
			let data = std::fs::File::open(path)?;
			let mut itp = tracing::info_span!("parse_png").in_scope(|| {
				Ok(crate::itp_png::png_to_itp(
					args,
					&png::read(&data).strict()?,
				))
			})?;
			crate::guess_itp_revision(args, &mut itp);
			cradle::itp::write(&itp)?
		}

		Some("dds") => {
			let data = std::fs::File::open(path)?;
			let mut itp = tracing::info_span!("parse_dds")
				.in_scope(|| crate::itp_dds::dds_to_itp(args, &data))?;
			crate::guess_itp_revision(args, &mut itp);
			cradle::itp::write(&itp)?
		}

		Some("itp") => std::fs::read(path)?,

		_ => eyre::bail!("unknown file extension"),
	};
	Ok(data)
}

fn pad(png: &mut png::Png, x: isize, y: isize, w: usize, h: usize) {
	match png {
		png::Png::Argb32(data) => {
			if let [data] = data.as_mut_slice() {
				*data = do_pad(data, x, y, w, h);
			}
		}
		png::Png::Indexed(_, data) => {
			if let [data] = data.as_mut_slice() {
				*data = do_pad(data, x, y, w, h);
			}
		}
	}
}

fn crop(png: &mut png::Png) -> (isize, isize) {
	match png {
		png::Png::Argb32(data) => {
			if let [data] = data.as_mut_slice() {
				let (out, offset) = do_crop(data);
				*data = out;
				offset
			} else {
				(0, 0)
			}
		}
		png::Png::Indexed(_, data) => {
			if let [data] = data.as_mut_slice() {
				let (out, offset) = do_crop(data);
				*data = out;
				offset
			} else {
				(0, 0)
			}
		}
	}
}

fn do_pad<T: PartialEq + Clone>(
	data: &Raster<T>,
	cx: isize,
	cy: isize,
	w: usize,
	h: usize,
) -> Raster<T> {
	let ox = ((w - data.width()) as isize / 2 - cx) as usize;
	let oy = ((h - data.height()) as isize / 2 - cy) as usize;
	let mut dst = Raster::splat(w, h, data[[0, 0]].clone());
	for x in 0..data.width() {
		for y in 0..data.height() {
			dst[[ox + x, oy + y]] = data[[x, y]].clone()
		}
	}
	dst
}

fn do_crop<T: PartialEq + Clone>(data: &Raster<T>) -> (Raster<T>, (isize, isize)) {
	let w = data.width();
	let h = data.height();

	let (l, r, u, d) = bounds(data);

	let ow = (r - l + 6).next_power_of_two();
	let oh = (d - u + 6).next_power_of_two();

	#[rustfmt::skip]
	let cx = if ow == w { 0 } else { w as isize / 2 - (r+l) as isize / 2 };
	#[rustfmt::skip]
	let cy = if oh == h { 0 } else { h as isize / 2 - (d+u) as isize / 2 };

	let ox = (w as isize / 2 - cx) as usize - ow / 2;
	let oy = (h as isize / 2 - cy) as usize - oh / 2;

	let mut dst = Raster::splat(ow, oh, data[[0, 0]].clone());
	for x in 0..ow {
		for y in 0..oh {
			dst[[x, y]] = data[[ox + x, oy + y]].clone()
		}
	}
	(dst, (cx, cy))
}

fn bounds<T: PartialEq>(data: &Raster<T>) -> (usize, usize, usize, usize) {
	let w = data.width();
	let h = data.height();

	let l = (0..w).find(|&x| (0..h).any(|y| data[[x, y]] != data[[0, 0]]));
	let r = (0..w).rfind(|&x| (0..h).any(|y| data[[x, y]] != data[[0, 0]]));
	let u = (0..h).find(|&y| (0..w).any(|x| data[[x, y]] != data[[0, 0]]));
	let d = (0..h).rfind(|&y| (0..w).any(|x| data[[x, y]] != data[[0, 0]]));

	let (Some(l), Some(r), Some(u), Some(d)) = (l, r, u, d) else {
		panic!("not sure what to do with empty images")
	};
	(l, r, u, d)
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

// Crop/pad currently do not roundtrip
#[cfg(false)]
#[cfg(test)]
#[filetest::filetest("../../samples/itc/*.itc")]
fn test_crop_pad(path: &Utf8Path, bytes: &[u8]) -> Result<(), eyre::Error> {
	let tmpdir = camino_tempfile::Builder::new()
		.prefix("cradle-")
		.suffix(&format!("-{}", path.file_stem().unwrap()))
		.tempdir()?;
	let args = &Args::default();

	let mut itc = cradle::itc::read(bytes)?;
	extract(args, &itc, Output::At(tmpdir.path().to_path_buf()))?;
	let file = std::fs::File::open(tmpdir.path().join("cradle.itc.json"))?;
	let mut itc2 = create(args, serde_json::from_reader(file)?, tmpdir.path())?;
	for (f1, f2) in itc.frames.iter_mut().zip(itc2.frames.iter_mut()) {
		if let (Some(itp1), Some(itp2)) = (f1.itp.take(), f2.itp.take()) {
			let mut itp1 = cradle::itp::read(&itp1)?;
			let mut itp2 = cradle::itp::read(&itp2)?;
			if let ImageData::Indexed(pal @ Palette::External(_), _) = &mut itp1.data {
				*pal = Palette::Embedded(itc.palette.clone().unwrap());
			}
			if let ImageData::Indexed(pal @ Palette::External(_), _) = &mut itp2.data {
				*pal = Palette::Embedded(itc.palette.clone().unwrap());
			}
			assert_eq!(itp1.data, itp2.data)
		}
	}
	assert_eq!(itc.frames, itc2.frames);
	Ok(())
}
