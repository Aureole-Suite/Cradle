use std::{path::Path, panic::catch_unwind};

use indicatif::ParallelProgressIterator;
use rayon::prelude::*;

fn main() -> anyhow::Result<()> {
	let stdout = std::process::Command::new("locate")
		.arg(".itp")
		.output()?
		.stdout;
	let stdout = String::from_utf8(stdout)?;
	let stdout = stdout.lines()
		.filter(|line| line.ends_with(".itp"))
		.collect::<Vec<_>>();

	let style = indicatif::ProgressStyle::with_template("{bar} {pos}/{len}")?
		.progress_chars("█🮆🮅🮄▀🮃🮂▔ ");
	let bar = indicatif::ProgressBar::new(stdout.len() as _)
		.with_style(style);
	stdout.par_iter().progress_with(bar.clone()).try_for_each(|line| {
		let path = Path::new(line);
		if path.is_file() {
			let file = std::fs::File::open(path)?;
			let dat = unsafe { memmap2::Mmap::map(&file)? };
			let v = catch_unwind(|| cradle::itp::read(&dat));
			let err = match v {
				Ok(Ok(_)) => None,
				Ok(Err(cradle::itp::Error::NotItp)) => None,
				Ok(Err(e)) => Some(anyhow::Error::from(e)),
				Err(_) => Some(anyhow::anyhow!("panic")),
			};
			if let Some(err) = err {
				bar.suspend(|| {
					println!("{} {:?}", path.display(), err);
				})
			}
		}
		anyhow::Ok(())
	})?;

	Ok(())
}
