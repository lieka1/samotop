pub mod dirmail;
pub mod mapper;

#[cfg(feature = "spf")]
pub use samotop_with_spf as spf;

#[cfg(feature = "lmtp-dispatch")]
pub use samotop_to_lmtp as lmtp;

pub use samotop_core::mail::*;