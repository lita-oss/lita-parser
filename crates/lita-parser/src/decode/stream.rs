#[cfg(feature = "decode")]
use encoding_rs::{
    Decoder as RsDecoder, DecoderResult, Encoding as RsEncoding, UTF_8, UTF_16BE, UTF_16LE,
};

use unicode_bom::Bom;

#[cfg(any(feature = "decode", feature = "tokio-io"))]
use std::io::Error as IoError;
use std::io::Result as IoResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeResult {
    InputEmpty,
    OutputFull,
    Malformed,
}

pub trait Decoder: Unpin {
    fn decode(&mut self, bytes: &[u8], buf: &mut [u8], last: bool) -> (DecodeResult, usize, usize);
}

#[cfg(feature = "decode")]
impl Decoder for RsDecoder {
    fn decode(&mut self, bytes: &[u8], buf: &mut [u8], last: bool) -> (DecodeResult, usize, usize) {
        let (res, read, written) = self.decode_to_utf8_without_replacement(bytes, buf, last);
        let res = match res {
            DecoderResult::InputEmpty => DecodeResult::InputEmpty,
            DecoderResult::OutputFull => DecodeResult::OutputFull,
            DecoderResult::Malformed(..) => DecodeResult::Malformed,
        };
        (res, read, written)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Utf8Decoder {
    bytes: [u8; 3],
    len: u8,
}

impl Decoder for Utf8Decoder {
    fn decode(&mut self, bytes: &[u8], buf: &mut [u8], last: bool) -> (DecodeResult, usize, usize) {
        use std::str::from_utf8;

        let mut read = 0;
        let mut written = 0;
        if self.len != 0 {
            let mut pending = [0; 4];
            let len = usize::from(self.len);
            pending[..len].copy_from_slice(&self.bytes[..len]);
            read = (pending[0].leading_ones() as usize - len).min(bytes.len());
            pending[len..len + read].copy_from_slice(&bytes[..read]);
            let len = len + read;
            match from_utf8(&pending[..len]) {
                Ok(_) if buf.len() < len => return (DecodeResult::OutputFull, 0, 0),
                Ok(_) => {
                    buf[..len].copy_from_slice(&pending[..len]);
                    written = len;
                }
                Err(e) => {
                    if let Some(len) = e.error_len() {
                        let read = len.saturating_sub(usize::from(self.len));
                        self.len = 0;
                        return (DecodeResult::Malformed, read, 0);
                    }
                    if last {
                        self.len = 0;
                        return (DecodeResult::Malformed, read, 0);
                    }
                    self.bytes[..len].copy_from_slice(&pending[..len]);
                    self.len = len as u8;
                    return (DecodeResult::InputEmpty, read, 0);
                }
            }
            self.len = 0;
        }
        let bytes = &bytes[read..];
        let buf = &mut buf[written..];
        let len = bytes.len().min(buf.len());
        let (res, consumed, len) = match from_utf8(&bytes[..len]) {
            Ok(_) => (
                if len < bytes.len() {
                    DecodeResult::OutputFull
                } else {
                    DecodeResult::InputEmpty
                },
                len,
                len,
            ),
            Err(e) => {
                let valid = e.valid_up_to();
                let (res, read) = if let Some(len) = e.error_len() {
                    (DecodeResult::Malformed, valid + len)
                } else if len < bytes.len() {
                    (DecodeResult::OutputFull, valid)
                } else if last {
                    (DecodeResult::Malformed, len)
                } else {
                    let bytes = &bytes[valid..];
                    self.bytes[..bytes.len()].copy_from_slice(bytes);
                    self.len = bytes.len() as u8;
                    (DecodeResult::InputEmpty, len)
                };
                (res, read, valid)
            }
        };
        buf[..len].copy_from_slice(&bytes[..len]);
        (res, read + consumed, written + len)
    }
}

pub trait MakeStreamDecoder: Unpin {
    type Decoder: Decoder;

    fn make_decoder(
        &mut self,
        bytes: &[u8],
        last: bool,
        full: bool,
    ) -> IoResult<Option<(Self::Decoder, usize)>>;
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Utf8;

impl MakeStreamDecoder for Utf8 {
    type Decoder = Utf8Decoder;

    fn make_decoder(
        &mut self,
        bytes: &[u8],
        last: bool,
        _full: bool,
    ) -> IoResult<Option<(Self::Decoder, usize)>> {
        let bom = &[0xef, 0xbb, 0xbf];
        if !last && bytes.len() < bom.len() && bom.starts_with(bytes) {
            return Ok(None);
        }
        let len = if Bom::from(bytes) == Bom::Utf8 { 3 } else { 0 };
        Ok(Some((Utf8Decoder::default(), len)))
    }
}

#[cfg(feature = "decode")]
impl MakeStreamDecoder for &'static RsEncoding {
    type Decoder = RsDecoder;

    fn make_decoder(
        &mut self,
        bytes: &[u8],
        last: bool,
        _full: bool,
    ) -> IoResult<Option<(Self::Decoder, usize)>> {
        let enc = *self;
        Ok(bom_len(enc, bytes, last).map(|len| (enc.new_decoder_without_bom_handling(), len)))
    }
}

#[cfg(feature = "decode")]
impl<const N: usize> MakeStreamDecoder for [&'static RsEncoding; N] {
    type Decoder = RsDecoder;

    fn make_decoder(
        &mut self,
        bytes: &[u8],
        last: bool,
        full: bool,
    ) -> IoResult<Option<(Self::Decoder, usize)>> {
        if self.is_empty() {
            return Err(invalid_data());
        }
        if !bom_ready(bytes, last) {
            return Ok(None);
        }
        let bom = match Bom::from(bytes) {
            Bom::Utf8 => Some(UTF_8),
            Bom::Utf16Be => Some(UTF_16BE),
            Bom::Utf16Le => Some(UTF_16LE),
            _ => None,
        };
        let enc = if let Some(enc) = bom.filter(|enc| self.contains(enc)) {
            enc
        } else {
            let mut candidates = self.iter().copied().filter(|enc| valid(bytes, enc, last));
            let Some(enc) = candidates.next() else {
                return Err(invalid_data());
            };
            if candidates.any(|other| other != enc) && !full && !last {
                return Ok(None);
            }
            enc
        };
        let Some(len) = bom_len(enc, bytes, last) else {
            return Ok(None);
        };
        let len = if enc == UTF_8 { 0 } else { len };
        Ok(Some((enc.new_decoder_without_bom_handling(), len)))
    }
}

#[cfg(any(feature = "decode", feature = "tokio-io"))]
fn invalid_data() -> IoError {
    use std::io::ErrorKind;

    ErrorKind::InvalidData.into()
}

#[cfg(feature = "decode")]
fn bom_ready(bytes: &[u8], eof: bool) -> bool {
    eof || ![
        &[0xef, 0xbb, 0xbf][..],
        &[0xfe, 0xff][..],
        &[0xff, 0xfe, 0, 0][..],
        &[0, 0, 0xfe, 0xff][..],
    ]
    .iter()
    .any(|bom| bytes.len() < bom.len() && bom.starts_with(bytes))
}

#[cfg(feature = "decode")]
fn bom_len(enc: &'static RsEncoding, bytes: &[u8], eof: bool) -> Option<usize> {
    let prefix = if enc == UTF_8 {
        &[0xef, 0xbb, 0xbf][..]
    } else if enc == UTF_16BE {
        &[0xfe, 0xff][..]
    } else if enc == UTF_16LE {
        &[0xff, 0xfe, 0, 0][..]
    } else {
        return Some(0);
    };
    if !eof && bytes.len() < prefix.len() && prefix.starts_with(bytes) {
        return None;
    }
    Some(match Bom::from(bytes) {
        Bom::Utf8 if enc == UTF_8 => 3,
        Bom::Utf16Be if enc == UTF_16BE => 2,
        Bom::Utf16Le if enc == UTF_16LE => 2,
        _ => 0,
    })
}

#[cfg(feature = "decode")]
fn valid(bytes: &[u8], enc: &'static RsEncoding, last: bool) -> bool {
    let mut dec = enc.new_decoder_without_bom_handling();
    let mut bytes = bytes;
    let mut buf = [0; 16];
    loop {
        let (res, read, _) = dec.decode_to_utf8_without_replacement(bytes, &mut buf, last);
        bytes = &bytes[read..];
        match res {
            DecoderResult::InputEmpty => return true,
            DecoderResult::Malformed(..) => return false,
            DecoderResult::OutputFull => {}
        }
    }
}

#[cfg(feature = "tokio-io")]
pub use tokio::{StreamDecoder, StreamDecoderUtf8};

#[cfg(feature = "tokio-io")]
mod tokio {
    use super::super::OneshotDecoder;
    use super::{DecodeResult, Decoder as _, MakeStreamDecoder, invalid_data};

    use ::tokio::io::{AsyncRead, ReadBuf};

    use std::io::Result as IoResult;
    use std::marker::PhantomData;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    const BUFFER_SIZE: usize = 4096;

    struct Input<R> {
        rdr: R,
        bytes: [u8; BUFFER_SIZE],
        pos: usize,
        len: usize,
        eof: bool,
    }

    impl<R> Input<R> {
        fn new(rdr: R) -> Self {
            Self {
                rdr,
                bytes: [0; BUFFER_SIZE],
                pos: 0,
                len: 0,
                eof: false,
            }
        }

        fn remaining(&self) -> &[u8] {
            &self.bytes[self.pos..self.len]
        }
    }

    impl<R: AsyncRead + Unpin> Input<R> {
        fn poll_fill(&mut self, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
            if self.eof {
                return Poll::Ready(Ok(()));
            }
            if self.pos != 0 {
                self.bytes.copy_within(self.pos..self.len, 0);
                self.len -= self.pos;
                self.pos = 0;
            }
            if self.len == BUFFER_SIZE {
                return Poll::Ready(Err(invalid_data()));
            }
            let mut buf = ReadBuf::new(&mut self.bytes[self.len..]);
            match Pin::new(&mut self.rdr).poll_read(cx, &mut buf) {
                Poll::Ready(Ok(())) => {
                    let len = buf.filled().len();
                    self.len += len;
                    self.eof = len == 0;
                    Poll::Ready(Ok(()))
                }
                Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                Poll::Pending => Poll::Pending,
            }
        }
    }

    struct Output<const N: usize> {
        bytes: [u8; N],
        pos: usize,
        len: usize,
    }

    impl<const N: usize> Output<N> {
        fn new() -> Self {
            Self {
                bytes: [0; N],
                pos: 0,
                len: 0,
            }
        }

        fn emit(&mut self, buf: &mut ReadBuf<'_>) -> bool {
            if self.pos == self.len {
                self.pos = 0;
                self.len = 0;
                return false;
            }
            let len = buf.remaining().min(self.len - self.pos);
            buf.put_slice(&self.bytes[self.pos..self.pos + len]);
            self.pos += len;
            true
        }
    }

    pub struct StreamDecoder<R, M: MakeStreamDecoder> {
        input: Input<R>,
        output: Output<16>,
        maker: M,
        decoder: Option<M::Decoder>,
        finished: bool,
        failed: bool,
    }

    impl<R, M: MakeStreamDecoder> StreamDecoder<R, M> {
        pub fn new(rdr: R, maker: M) -> Self {
            Self {
                input: Input::new(rdr),
                output: Output::new(),
                maker,
                decoder: None,
                finished: false,
                failed: false,
            }
        }

        fn initialize(&mut self) -> IoResult<bool> {
            let bytes = self.input.remaining();
            match self
                .maker
                .make_decoder(bytes, self.input.eof, bytes.len() == BUFFER_SIZE)?
            {
                Some((dec, len)) if len <= bytes.len() => {
                    self.input.pos += len;
                    self.decoder = Some(dec);
                    Ok(true)
                }
                Some(_) => Err(invalid_data()),
                None if self.input.eof || bytes.len() == BUFFER_SIZE => Err(invalid_data()),
                None => Ok(false),
            }
        }
    }

    impl<R: AsyncRead + Unpin, M: MakeStreamDecoder> AsyncRead for StreamDecoder<R, M> {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<IoResult<()>> {
            let this = self.get_mut();
            if buf.remaining() == 0 {
                return Poll::Ready(Ok(()));
            }
            loop {
                if this.output.emit(buf) {
                    return Poll::Ready(Ok(()));
                }
                if this.failed {
                    return Poll::Ready(Err(invalid_data()));
                }
                if this.finished {
                    return Poll::Ready(Ok(()));
                }
                if this.decoder.is_none() {
                    match this.initialize() {
                        Ok(true) => {}
                        Ok(false) => {
                            match this.input.poll_fill(cx) {
                                Poll::Ready(Ok(())) => {}
                                res => return res,
                            }
                            continue;
                        }
                        Err(e) => {
                            this.failed = true;
                            return Poll::Ready(Err(e));
                        }
                    }
                }
                let Some(dec) = this.decoder.as_mut() else {
                    return Poll::Ready(Err(invalid_data()));
                };
                let direct = buf.remaining() >= 4;
                let len = if direct {
                    buf.remaining().min(BUFFER_SIZE * 3 + 4)
                } else {
                    this.output.bytes.len()
                };
                let (res, read, written) = if direct {
                    dec.decode(
                        this.input.remaining(),
                        buf.initialize_unfilled_to(len),
                        this.input.eof,
                    )
                } else {
                    dec.decode(
                        this.input.remaining(),
                        &mut this.output.bytes,
                        this.input.eof,
                    )
                };
                if read > this.input.remaining().len() || written > len {
                    this.failed = true;
                    return Poll::Ready(Err(invalid_data()));
                }
                this.input.pos += read;
                if !direct {
                    this.output.len = written;
                }
                match res {
                    DecodeResult::InputEmpty => {
                        this.failed = this.input.eof && this.input.pos != this.input.len;
                        this.finished = this.input.eof && !this.failed;
                    }
                    DecodeResult::Malformed => this.failed = true,
                    DecodeResult::OutputFull if read == 0 && written == 0 => this.failed = true,
                    DecodeResult::OutputFull => {}
                }
                if direct && written != 0 {
                    buf.advance(written);
                    return Poll::Ready(Ok(()));
                }
                if written != 0 || this.failed || this.finished {
                    continue;
                }
                match this.input.poll_fill(cx) {
                    Poll::Ready(Ok(())) => {}
                    res => return res,
                }
            }
        }
    }

    pub struct StreamDecoderUtf8<R, D: OneshotDecoder> {
        input: Input<R>,
        output: Output<4>,
        first: bool,
        strip_bom: bool,
        failed: bool,
        decoder: PhantomData<fn() -> D>,
    }

    impl<R, D: OneshotDecoder> StreamDecoderUtf8<R, D> {
        pub fn new(rdr: R, strip_bom: bool) -> Self {
            Self {
                input: Input::new(rdr),
                output: Output::new(),
                first: true,
                strip_bom,
                failed: false,
                decoder: PhantomData,
            }
        }
    }

    impl<R, D: OneshotDecoder> StreamDecoderUtf8<R, D> {
        fn reject(&mut self, buf: &ReadBuf<'_>, filled: usize) -> Poll<IoResult<()>> {
            self.failed = true;
            Poll::Ready(if buf.filled().len() == filled {
                Err(invalid_data())
            } else {
                Ok(())
            })
        }
    }

    impl<R: AsyncRead + Unpin, D: OneshotDecoder> AsyncRead for StreamDecoderUtf8<R, D> {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<IoResult<()>> {
            use std::str::from_utf8;

            let this = self.get_mut();
            if buf.remaining() == 0 {
                return Poll::Ready(Ok(()));
            }
            let filled = buf.filled().len();
            loop {
                if this.output.emit(buf) || buf.remaining() == 0 {
                    return Poll::Ready(Ok(()));
                }
                if this.failed {
                    return this.reject(buf, filled);
                }
                if this.input.pos == this.input.len {
                    if this.input.eof || buf.filled().len() != filled {
                        return Poll::Ready(Ok(()));
                    }
                    match this.input.poll_fill(cx) {
                        Poll::Ready(Ok(())) => {}
                        res => return res,
                    }
                    continue;
                }
                let bytes = this.input.remaining();
                let len = match bytes[0] {
                    0..=0x7f => 1,
                    0xc2..=0xdf => 2,
                    0xe0..=0xef => 3,
                    0xf0..=0xf4 => 4,
                    _ => return this.reject(buf, filled),
                };
                if bytes.len() < len {
                    if this.input.eof {
                        return this.reject(buf, filled);
                    }
                    if buf.filled().len() != filled {
                        return Poll::Ready(Ok(()));
                    }
                    match this.input.poll_fill(cx) {
                        Poll::Ready(Ok(())) => {}
                        res => return res,
                    }
                    continue;
                }
                let Ok(s) = from_utf8(&bytes[..len]) else {
                    return this.reject(buf, filled);
                };
                if !D::decodable(s) {
                    return this.reject(buf, filled);
                }
                if !(this.first && this.strip_bom && s == "\u{feff}") {
                    if buf.remaining() >= len {
                        buf.put_slice(&bytes[..len]);
                    } else {
                        this.output.bytes[..len].copy_from_slice(&bytes[..len]);
                        this.output.len = len;
                    }
                }
                this.first = false;
                this.input.pos += len;
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::super::{MakeStreamDecoder, Utf8};
        use super::StreamDecoder;

        use ::tokio::io::{AsyncRead, AsyncReadExt as _};

        use std::io::{ErrorKind, Result as IoResult};

        async fn read(mut rdr: impl AsyncRead + Unpin) -> IoResult<String> {
            let mut buf = String::new();
            rdr.read_to_string(&mut buf).await?;
            Ok(buf)
        }

        async fn check(enc: impl MakeStreamDecoder, bytes: &[u8], expected: &str) {
            assert_eq!(
                read(StreamDecoder::new(bytes, enc)).await.unwrap(),
                expected
            );
        }

        #[::tokio::test]
        async fn accepts_empty_utf8_input() {
            use crate::decode::{EncodingTokioExt as _, Utf8 as EncodingUtf8};

            check(Utf8, b"", "").await;
            assert_eq!(
                read(EncodingUtf8.stream_decoder_utf8(&b""[..]))
                    .await
                    .unwrap(),
                ""
            );
            #[cfg(feature = "decode")]
            {
                use crate::decode::{Dynamic, StreamEncoding as _};

                use encoding_rs::{EUC_JP, SHIFT_JIS, UTF_16BE, UTF_16LE};

                for enc in [SHIFT_JIS, EUC_JP, UTF_16BE, UTF_16LE] {
                    check(enc, b"", "").await;
                }
                check(Dynamic.stream_decoder_maker(), b"", "").await;
            }
        }

        #[::tokio::test]
        async fn decodes_utf8() {
            check(Utf8, "春はあけぼの。".as_bytes(), "春はあけぼの。").await;
        }

        #[cfg(feature = "decode")]
        #[::tokio::test]
        async fn decodes_supported_charsets() {
            use crate::decode::{Dynamic, StreamEncoding as _};

            use encoding_rs::{EUC_JP, SHIFT_JIS, UTF_16BE, UTF_16LE};

            let utf8_bytes = "春はあけぼの。";
            let shift_jis_bytes = &[
                0x8F, 0x74, 0x82, 0xCD, 0x82, 0xA0, 0x82, 0xAF, 0x82, 0xDA, 0x82, 0xCC, 0x81, 0x42,
            ];
            let euc_jp_bytes = &[
                0xBD, 0xD5, 0xA4, 0xCF, 0xA4, 0xA2, 0xA4, 0xB1, 0xA4, 0xDC, 0xA4, 0xCE, 0xA1, 0xA3,
            ];
            let utf16_be_bytes = &[
                0x66, 0x25, 0x30, 0x6F, 0x30, 0x42, 0x30, 0x51, 0x30, 0x7C, 0x30, 0x6E, 0x30, 0x02,
            ];
            let utf16_le_bytes = &[
                0x25, 0x66, 0x6F, 0x30, 0x42, 0x30, 0x51, 0x30, 0x7C, 0x30, 0x6E, 0x30, 0x02, 0x30,
            ];
            check(SHIFT_JIS, shift_jis_bytes, utf8_bytes).await;
            check(EUC_JP, euc_jp_bytes, utf8_bytes).await;
            check(UTF_16BE, utf16_be_bytes, utf8_bytes).await;
            check(UTF_16LE, utf16_le_bytes, utf8_bytes).await;
            check(
                Dynamic.stream_decoder_maker(),
                utf8_bytes.as_bytes(),
                utf8_bytes,
            )
            .await;
            check(Dynamic.stream_decoder_maker(), shift_jis_bytes, utf8_bytes).await;
            check(SHIFT_JIS, euc_jp_bytes, "ｽﾕ､ﾏ､｢､ｱ､ﾜ､ﾎ｡｣").await;
            check(
                Dynamic.stream_decoder_maker(),
                euc_jp_bytes,
                "ｽﾕ､ﾏ､｢､ｱ､ﾜ､ﾎ｡｣",
            )
            .await;
        }

        #[::tokio::test]
        async fn removes_matching_boms() {
            use crate::decode::{EncodingTokioExt as _, Utf8 as EncodingUtf8};

            assert_eq!(
                read(EncodingUtf8.stream_decoder_utf8(&b"\xEF\xBB\xBFa"[..]))
                    .await
                    .unwrap(),
                "a"
            );
            check(Utf8, &[0xEF, 0xBB, 0xBF, 0x61], "a").await;
            #[cfg(feature = "decode")]
            {
                use crate::decode::{Dynamic, StreamEncoding as _};

                use encoding_rs::{UTF_16BE, UTF_16LE};

                check(UTF_16BE, &[0xFE, 0xFF, 0x00, 0x61], "a").await;
                check(UTF_16LE, &[0xFF, 0xFE, 0x61, 0x00], "a").await;
                check(
                    Dynamic.stream_decoder_maker(),
                    &[0xEF, 0xBB, 0xBF, 0x61],
                    "\u{feff}a",
                )
                .await;
            }
        }

        #[cfg(feature = "decode")]
        #[::tokio::test]
        async fn checks_decodable_charsets() {
            use crate::decode::{
                Dynamic, EncodingTokioExt as _, EucJp, ShiftJis, Utf8, Utf16Be, Utf16Le,
            };

            use std::array::from_fn;

            let bytes: [u8; 128] = from_fn(|i| i as u8);
            assert_eq!(
                read(Utf8.stream_decoder_utf8(&bytes[..]))
                    .await
                    .unwrap()
                    .as_bytes(),
                bytes
            );
            assert_eq!(
                read(Dynamic.stream_decoder_utf8(&bytes[..]))
                    .await
                    .unwrap()
                    .as_bytes(),
                bytes
            );
            assert_eq!(
                read(ShiftJis.stream_decoder_utf8(&bytes[..]))
                    .await
                    .unwrap()
                    .as_bytes(),
                bytes
            );
            assert_eq!(
                read(EucJp.stream_decoder_utf8(&bytes[..]))
                    .await
                    .unwrap()
                    .as_bytes(),
                bytes
            );
            assert_eq!(
                read(Utf16Be.stream_decoder_utf8(&bytes[..]))
                    .await
                    .unwrap_err()
                    .kind(),
                ErrorKind::InvalidData
            );
            assert_eq!(
                read(Utf16Le.stream_decoder_utf8(&bytes[..]))
                    .await
                    .unwrap_err()
                    .kind(),
                ErrorKind::InvalidData
            );
            for bytes in [
                "春はあけぼの。",
                "é",
                "aé",
                "🌸",
                "\u{80}",
                "\u{fffd}",
                "\u{feff}",
            ] {
                assert_eq!(
                    read(Utf8.stream_decoder_utf8(bytes.as_bytes()))
                        .await
                        .unwrap(),
                    bytes.trim_start_matches('\u{feff}')
                );
                assert_eq!(
                    read(Dynamic.stream_decoder_utf8(bytes.as_bytes()))
                        .await
                        .unwrap(),
                    bytes
                );
                assert_eq!(
                    read(ShiftJis.stream_decoder_utf8(bytes.as_bytes()))
                        .await
                        .unwrap_err()
                        .kind(),
                    ErrorKind::InvalidData
                );
                assert_eq!(
                    read(EucJp.stream_decoder_utf8(bytes.as_bytes()))
                        .await
                        .unwrap_err()
                        .kind(),
                    ErrorKind::InvalidData
                );
                assert_eq!(
                    read(Utf16Be.stream_decoder_utf8(bytes.as_bytes()))
                        .await
                        .unwrap_err()
                        .kind(),
                    ErrorKind::InvalidData
                );
                assert_eq!(
                    read(Utf16Le.stream_decoder_utf8(bytes.as_bytes()))
                        .await
                        .unwrap_err()
                        .kind(),
                    ErrorKind::InvalidData
                );
            }
            assert_eq!(read(Utf8.stream_decoder_utf8(&b""[..])).await.unwrap(), "");
            assert_eq!(
                read(Dynamic.stream_decoder_utf8(&b""[..])).await.unwrap(),
                ""
            );
            assert_eq!(
                read(ShiftJis.stream_decoder_utf8(&b""[..])).await.unwrap(),
                ""
            );
            assert_eq!(read(EucJp.stream_decoder_utf8(&b""[..])).await.unwrap(), "");
            assert_eq!(
                read(Utf16Be.stream_decoder_utf8(&b""[..])).await.unwrap(),
                ""
            );
            assert_eq!(
                read(Utf16Le.stream_decoder_utf8(&b""[..])).await.unwrap(),
                ""
            );
        }

        #[::tokio::test]
        async fn decodes_split_utf8_sequences() {
            use crate::decode::{EncodingTokioExt as _, Utf8};

            async fn read(mut rdr: impl AsyncRead + Unpin) -> Vec<u8> {
                let mut bytes = Vec::new();
                let mut buf = [0; 1];
                loop {
                    let len = rdr.read(&mut buf).await.unwrap();
                    if len == 0 {
                        return bytes;
                    }
                    bytes.extend_from_slice(&buf[..len]);
                }
            }

            for bytes in ["é", "あ", "🌸", "あいうえお"] {
                let bytes = bytes.as_bytes();
                for len in 0..=bytes.len() {
                    let rdr = (&bytes[..len]).chain(&bytes[len..]);
                    assert_eq!(read(Utf8.stream_decoder(rdr)).await, bytes);
                    let rdr = (&bytes[..len]).chain(&bytes[len..]);
                    assert_eq!(read(Utf8.stream_decoder_utf8(rdr)).await, bytes);
                }
            }
        }

        #[::tokio::test]
        async fn rejects_malformed_utf8() {
            use crate::decode::{EncodingTokioExt as _, Utf8};

            for bytes in [
                &b"\xFF"[..],
                b"\xC0\xAF",
                b"\xED\xA0\x80",
                b"\xF4\x90\x80\x80",
                b"\xE3\x81",
            ] {
                for len in 0..=bytes.len() {
                    let rdr = (&bytes[..len]).chain(&bytes[len..]);
                    assert_eq!(
                        read(Utf8.stream_decoder(rdr)).await.unwrap_err().kind(),
                        ErrorKind::InvalidData
                    );
                    let rdr = (&bytes[..len]).chain(&bytes[len..]);
                    assert_eq!(
                        read(Utf8.stream_decoder_utf8(rdr))
                            .await
                            .unwrap_err()
                            .kind(),
                        ErrorKind::InvalidData
                    );
                }
            }
        }

        #[cfg(feature = "decode")]
        #[::tokio::test]
        async fn fixes_dynamic_encoding_when_buffer_is_full() {
            use crate::decode::{Dynamic, EncodingTokioExt as _};

            use super::BUFFER_SIZE;

            let mut bytes = vec![b'a'; BUFFER_SIZE];
            bytes.extend_from_slice(&[0x82, 0xA0]);
            assert_eq!(
                read(Dynamic.stream_decoder(bytes.as_slice()))
                    .await
                    .unwrap_err()
                    .kind(),
                ErrorKind::InvalidData
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DecodeResult, Decoder as _, MakeStreamDecoder, Utf8, Utf8Decoder};

    fn check(mut enc: impl MakeStreamDecoder, bytes: &[u8], expected: &str) {
        let (mut dec, read) = enc.make_decoder(bytes, true, false).unwrap().unwrap();
        let bytes = &bytes[read..];
        let mut buf = [0; 128];
        let (res, read, written) = dec.decode(bytes, &mut buf, true);
        assert_eq!(res, DecodeResult::InputEmpty);
        assert_eq!(read, bytes.len());
        assert_eq!(&buf[..written], expected.as_bytes());
    }

    #[test]
    fn accepts_empty_utf8_input() {
        check(Utf8, b"", "");
        #[cfg(feature = "decode")]
        {
            use crate::decode::{Dynamic, StreamEncoding as _};

            use encoding_rs::{EUC_JP, SHIFT_JIS, UTF_16BE, UTF_16LE};

            for enc in [SHIFT_JIS, EUC_JP, UTF_16BE, UTF_16LE] {
                check(enc, b"", "");
            }
            check(Dynamic.stream_decoder_maker(), b"", "");
        }
    }

    #[test]
    fn decodes_utf8() {
        check(Utf8, "春はあけぼの。".as_bytes(), "春はあけぼの。");
    }

    #[cfg(feature = "decode")]
    #[test]
    fn decodes_supported_charsets() {
        use crate::decode::{Dynamic, StreamEncoding as _};

        use encoding_rs::{EUC_JP, SHIFT_JIS, UTF_16BE, UTF_16LE};

        let utf8_bytes = "春はあけぼの。";
        let shift_jis_bytes = &[
            0x8F, 0x74, 0x82, 0xCD, 0x82, 0xA0, 0x82, 0xAF, 0x82, 0xDA, 0x82, 0xCC, 0x81, 0x42,
        ];
        let euc_jp_bytes = &[
            0xBD, 0xD5, 0xA4, 0xCF, 0xA4, 0xA2, 0xA4, 0xB1, 0xA4, 0xDC, 0xA4, 0xCE, 0xA1, 0xA3,
        ];
        let utf16_be_bytes = &[
            0x66, 0x25, 0x30, 0x6F, 0x30, 0x42, 0x30, 0x51, 0x30, 0x7C, 0x30, 0x6E, 0x30, 0x02,
        ];
        let utf16_le_bytes = &[
            0x25, 0x66, 0x6F, 0x30, 0x42, 0x30, 0x51, 0x30, 0x7C, 0x30, 0x6E, 0x30, 0x02, 0x30,
        ];
        check(SHIFT_JIS, shift_jis_bytes, utf8_bytes);
        check(EUC_JP, euc_jp_bytes, utf8_bytes);
        check(UTF_16BE, utf16_be_bytes, utf8_bytes);
        check(UTF_16LE, utf16_le_bytes, utf8_bytes);
        check(
            Dynamic.stream_decoder_maker(),
            utf8_bytes.as_bytes(),
            utf8_bytes,
        );
        check(Dynamic.stream_decoder_maker(), shift_jis_bytes, utf8_bytes);
        check(SHIFT_JIS, euc_jp_bytes, "ｽﾕ､ﾏ､｢､ｱ､ﾜ､ﾎ｡｣");
        check(
            Dynamic.stream_decoder_maker(),
            euc_jp_bytes,
            "ｽﾕ､ﾏ､｢､ｱ､ﾜ､ﾎ｡｣",
        );
    }

    #[test]
    fn removes_matching_boms() {
        check(Utf8, &[0xEF, 0xBB, 0xBF, 0x61], "a");
        #[cfg(feature = "decode")]
        {
            use crate::decode::{Dynamic, StreamEncoding as _};

            use encoding_rs::{UTF_16BE, UTF_16LE};

            check(UTF_16BE, &[0xFE, 0xFF, 0x00, 0x61], "a");
            check(UTF_16LE, &[0xFF, 0xFE, 0x61, 0x00], "a");
            check(
                Dynamic.stream_decoder_maker(),
                &[0xEF, 0xBB, 0xBF, 0x61],
                "\u{feff}a",
            );
        }
    }

    #[test]
    fn decodes_split_utf8_sequences() {
        for bytes in ["é", "あ", "🌸", "あいうえお"] {
            let bytes = bytes.as_bytes();
            for len in 0..=bytes.len() {
                let mut dec = Utf8Decoder::default();
                let mut buf = [0; 32];
                let (res, read, written) = dec.decode(&bytes[..len], &mut buf, false);
                assert_eq!(res, DecodeResult::InputEmpty);
                assert_eq!(read, len);
                let (res, read, remaining) = dec.decode(&bytes[len..], &mut buf[written..], true);
                assert_eq!(res, DecodeResult::InputEmpty);
                assert_eq!(read, bytes.len() - len);
                assert_eq!(&buf[..written + remaining], bytes);
            }
        }
    }

    #[test]
    fn retains_split_utf8_when_output_is_full() {
        let mut dec = Utf8Decoder::default();
        let mut buf = [0; 4];
        assert_eq!(
            dec.decode(&[0xE3, 0x81], &mut buf, false),
            (DecodeResult::InputEmpty, 2, 0)
        );
        assert_eq!(
            dec.decode(&[0x82], &mut buf[..2], true),
            (DecodeResult::OutputFull, 0, 0)
        );
        assert_eq!(
            dec.decode(&[0x82], &mut buf, true),
            (DecodeResult::InputEmpty, 1, 3)
        );
        assert_eq!(&buf[..3], "あ".as_bytes());
    }

    #[test]
    fn rejects_malformed_utf8() {
        for bytes in [
            &b"\xFF"[..],
            b"\xC0\xAF",
            b"\xED\xA0\x80",
            b"\xF4\x90\x80\x80",
            b"\xE3\x81",
        ] {
            for len in 0..=bytes.len() {
                let mut dec = Utf8Decoder::default();
                let mut buf = [0; 16];
                let (res, _, _) = dec.decode(&bytes[..len], &mut buf, false);
                if res != DecodeResult::Malformed {
                    assert_eq!(
                        dec.decode(&bytes[len..], &mut buf, true).0,
                        DecodeResult::Malformed
                    );
                }
            }
        }
    }
}
