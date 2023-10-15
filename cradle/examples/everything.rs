use std::panic::{catch_unwind, RefUnwindSafe};
use std::path::Path;

use indicatif::ParallelProgressIterator;
use rayon::prelude::*;

fn main() -> anyhow::Result<()> {
	run_everything(".itp", |dat| match cradle::itp::read(dat) {
		Ok(_) => Ok(()),
		Err(cradle::itp::Error::NotItp) => Ok(()),
		Err(e) => Err(e.into())
	})?;
	Ok(())
}

fn run_everything(
	suffix: &str,
	f: impl Fn(&[u8]) -> anyhow::Result<()> + Send + Sync + RefUnwindSafe,
) -> anyhow::Result<()> {
	let stdout = std::process::Command::new("locate")
		.arg(suffix)
		.output()?
		.stdout;
	let stdout = String::from_utf8(stdout)?;
	let stdout = stdout.lines()
		.filter(|line| line.ends_with(suffix))
		.collect::<Vec<_>>();

	let style = indicatif::ProgressStyle::with_template("{prefix} {bar} {pos}/{len}")?
		.progress_chars("█🮆🮅🮄▀🮃🮂▔ ");
	let bar = indicatif::ProgressBar::new(stdout.len() as _)
		.with_style(style)
		.with_prefix(suffix.to_owned());
	stdout.par_iter().progress_with(bar.clone()).try_for_each(|line| {
		let path = Path::new(line);
		if path.is_file() {
			let file = std::fs::File::open(path)?;
			let dat = unsafe { memmap2::Mmap::map(&file)? };
			let v = catch_unwind(|| f(&dat));
			let err = match v {
				Ok(Ok(())) => None,
				Ok(Err(e)) => Some(e),
				Err(_) => Some(anyhow::anyhow!("panic")),
			};
			if let Some(err) = err {
				bar.suspend(|| {
					eprintln!("{} {:?}", path.display(), err);
				})
			}
		}
		anyhow::Ok(())
	})?;

	Ok(())
}
