mod encoding;
mod error;
mod oneshot;
mod stream;

#[cfg(feature = "tokio-io")]
pub use encoding::EncodingTokioExt;
#[cfg(feature = "decode")]
pub use encoding::{Dynamic, EucJp, ShiftJis, Utf16Be, Utf16Le};
pub use encoding::{Encoding, StreamEncoding, Utf8};

pub use oneshot::Decoder as OneshotDecoder;

pub use stream::{DecodeResult, Decoder, MakeStreamDecoder, Utf8Decoder};
#[cfg(feature = "tokio-io")]
pub use stream::{StreamDecoder, StreamDecoderUtf8};

pub use error::UnknownEncoding;

#[cfg(any(feature = "txt-sync", feature = "csv-sync"))]
pub(crate) use oneshot::decode_cow;
#[cfg(any(feature = "txt-sync", feature = "csv-sync"))]
pub(crate) use oneshot::decode_owned;
