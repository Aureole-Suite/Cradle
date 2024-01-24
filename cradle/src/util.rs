use strict_result::{Strict as _, StrictResult};

pub macro bail {
	($fmt:literal $(, $e:expr)* $(,)?) => { bail!(format_args!($fmt$(, $e)*)) },
	($e:expr) => { { Err($e)?; loop {} } },
}

pub macro ensure {
	($e:expr) => { ensure!($e, format_args!("{}", stringify!($e))) },
	($e:expr, $($t:tt)*) => { if !($e) { bail!($($t)*) } },
	(let $pat:pat = $e:expr) => { ensure!(let $pat = $e, format_args!("{}", stringify!(let $pat = $e))) },
	(let $pat:pat = $e:expr, $($t:tt)*) => { let $pat = $e else { bail!($($t)*) }; },
}

#[extend::ext]
pub impl<T> Option<T> {
	fn or_whatever<E>(self, v: impl std::fmt::Display) -> StrictResult<T, E>
	where
		E: for<'a> From<std::fmt::Arguments<'a>>,
	{
		self.ok_or_else(move || E::from(format_args!("{}", v)))
			.strict()
	}
}
