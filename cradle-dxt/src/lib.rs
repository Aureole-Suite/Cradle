mod s3tc;
mod bc7;

pub use s3tc::bc1 as decode_bc1;
pub use s3tc::bc2 as decode_bc2;
pub use s3tc::bc3 as decode_bc3;

pub use bc7::decode as decode_bc7;
