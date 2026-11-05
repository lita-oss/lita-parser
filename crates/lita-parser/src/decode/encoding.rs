use super::{
    oneshot,
    stream::{self, MakeStreamDecoder, Utf8Decoder},
};

pub trait Encoding: Copy {
    const ASCII_COMPAT: bool;
    const UTF8_COMPAT: bool;
    const UTF8_INTERSECT: bool;

    type OneshotDecoder: oneshot::Decoder;

    fn oneshot_decoder(self) -> Self::OneshotDecoder;
}

pub trait StreamEncoding: Encoding {
    type StreamDecoder: stream::Decoder;
    type MakeStreamDecoder: MakeStreamDecoder<Decoder = Self::StreamDecoder>;

    fn stream_decoder_maker(self) -> Self::MakeStreamDecoder;
}

#[cfg(feature = "_tokio-io")]
pub use tokio::EncodingTokioExt;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Utf8;

impl Encoding for Utf8 {
    const ASCII_COMPAT: bool = true;
    const UTF8_COMPAT: bool = true;
    const UTF8_INTERSECT: bool = true;

    type OneshotDecoder = oneshot::Utf8;

    fn oneshot_decoder(self) -> Self::OneshotDecoder {
        Default::default()
    }
}

impl StreamEncoding for Utf8 {
    type StreamDecoder = Utf8Decoder;
    type MakeStreamDecoder = stream::Utf8;

    fn stream_decoder_maker(self) -> Self::MakeStreamDecoder {
        Default::default()
    }
}

#[cfg(feature = "decode")]
pub use other::{Dynamic, EucJp, ShiftJis, Utf16Be, Utf16Le};

#[cfg(feature = "decode")]
mod other {
    use super::{Encoding, StreamEncoding, oneshot};

    use encoding_rs::{Decoder as RsDecoder, Encoding as RsEncoding};

    #[derive(Debug, Default, Clone, Copy)]
    pub struct Utf16Be;

    impl Encoding for Utf16Be {
        const ASCII_COMPAT: bool = false;
        const UTF8_COMPAT: bool = false;
        const UTF8_INTERSECT: bool = false;

        type OneshotDecoder = oneshot::Utf16Be;

        fn oneshot_decoder(self) -> Self::OneshotDecoder {
            Default::default()
        }
    }

    impl StreamEncoding for Utf16Be {
        type StreamDecoder = RsDecoder;
        type MakeStreamDecoder = &'static RsEncoding;

        fn stream_decoder_maker(self) -> Self::MakeStreamDecoder {
            use encoding_rs::UTF_16BE;

            UTF_16BE
        }
    }

    #[derive(Debug, Default, Clone, Copy)]
    pub struct Utf16Le;

    impl Encoding for Utf16Le {
        const ASCII_COMPAT: bool = false;
        const UTF8_COMPAT: bool = false;
        const UTF8_INTERSECT: bool = false;

        type OneshotDecoder = oneshot::Utf16Le;

        fn oneshot_decoder(self) -> Self::OneshotDecoder {
            Default::default()
        }
    }

    impl StreamEncoding for Utf16Le {
        type StreamDecoder = RsDecoder;
        type MakeStreamDecoder = &'static RsEncoding;

        fn stream_decoder_maker(self) -> Self::MakeStreamDecoder {
            use encoding_rs::UTF_16LE;

            UTF_16LE
        }
    }

    #[derive(Debug, Default, Clone, Copy)]
    pub struct ShiftJis;

    impl Encoding for ShiftJis {
        const ASCII_COMPAT: bool = true;
        const UTF8_COMPAT: bool = false;
        const UTF8_INTERSECT: bool = true;

        type OneshotDecoder = oneshot::ShiftJis;

        fn oneshot_decoder(self) -> Self::OneshotDecoder {
            Default::default()
        }
    }

    impl StreamEncoding for ShiftJis {
        type StreamDecoder = RsDecoder;
        type MakeStreamDecoder = &'static RsEncoding;

        fn stream_decoder_maker(self) -> Self::MakeStreamDecoder {
            use encoding_rs::SHIFT_JIS;

            SHIFT_JIS
        }
    }

    #[derive(Debug, Default, Clone, Copy)]
    pub struct EucJp;

    impl Encoding for EucJp {
        const ASCII_COMPAT: bool = true;
        const UTF8_COMPAT: bool = false;
        const UTF8_INTERSECT: bool = true;

        type OneshotDecoder = oneshot::EucJp;

        fn oneshot_decoder(self) -> Self::OneshotDecoder {
            Default::default()
        }
    }

    impl StreamEncoding for EucJp {
        type StreamDecoder = RsDecoder;
        type MakeStreamDecoder = &'static RsEncoding;

        fn stream_decoder_maker(self) -> Self::MakeStreamDecoder {
            use encoding_rs::EUC_JP;

            EUC_JP
        }
    }

    #[derive(Debug, Default, Clone, Copy)]
    pub struct Dynamic;

    impl Encoding for Dynamic {
        const ASCII_COMPAT: bool = false;
        const UTF8_COMPAT: bool = false;
        const UTF8_INTERSECT: bool = true;

        type OneshotDecoder = oneshot::Dynamic;

        fn oneshot_decoder(self) -> Self::OneshotDecoder {
            Default::default()
        }
    }

    impl StreamEncoding for Dynamic {
        type StreamDecoder = RsDecoder;
        type MakeStreamDecoder = [&'static RsEncoding; 5];

        fn stream_decoder_maker(self) -> Self::MakeStreamDecoder {
            use encoding_rs::{EUC_JP, SHIFT_JIS, UTF_8, UTF_16BE, UTF_16LE};

            [UTF_8, SHIFT_JIS, EUC_JP, UTF_16BE, UTF_16LE]
        }
    }
}

#[cfg(feature = "_tokio-io")]
mod tokio {
    use super::super::stream::{StreamDecoder, StreamDecoderUtf8};
    use super::StreamEncoding;

    use ::tokio::io::AsyncRead;

    pub trait EncodingTokioExt: StreamEncoding {
        fn stream_decoder<R: AsyncRead + Unpin>(
            self,
            rdr: R,
        ) -> StreamDecoder<R, Self::MakeStreamDecoder>;

        fn stream_decoder_utf8<R: AsyncRead + Unpin>(
            self,
            rdr: R,
        ) -> StreamDecoderUtf8<R, Self::OneshotDecoder>;
    }

    impl<E: StreamEncoding> EncodingTokioExt for E {
        fn stream_decoder<R: AsyncRead + Unpin>(
            self,
            rdr: R,
        ) -> StreamDecoder<R, Self::MakeStreamDecoder> {
            StreamDecoder::new(rdr, self.stream_decoder_maker())
        }

        fn stream_decoder_utf8<R: AsyncRead + Unpin>(
            self,
            rdr: R,
        ) -> StreamDecoderUtf8<R, Self::OneshotDecoder> {
            StreamDecoderUtf8::new(rdr, Self::UTF8_COMPAT)
        }
    }
}
