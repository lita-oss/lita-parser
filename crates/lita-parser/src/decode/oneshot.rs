use super::UnknownEncoding;

use std::borrow::Cow;

pub trait Decoder {
    fn decodable(bytes: &str) -> bool;

    fn decode(self, bytes: &[u8]) -> Result<Cow<'_, str>, UnknownEncoding>;
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Utf8;

impl Decoder for Utf8 {
    fn decodable(_bytes: &str) -> bool {
        true
    }

    fn decode(self, bytes: &[u8]) -> Result<Cow<'_, str>, UnknownEncoding> {
        use unicode_bom::Bom;

        use std::str::from_utf8;

        let bytes = if Bom::from(bytes) == Bom::Utf8 {
            &bytes[3..]
        } else {
            bytes
        };

        from_utf8(bytes)
            .map(Cow::Borrowed)
            .map_err(|_| UnknownEncoding)
    }
}

#[cfg(feature = "decode")]
pub use other::{Dynamic, EucJp, ShiftJis, Utf16Be, Utf16Le};

#[cfg(feature = "decode")]
mod other {
    use super::{Decoder, UnknownEncoding};

    use encoding_rs::Encoding as RsEncoding;

    use unicode_bom::Bom;

    use std::borrow::Cow;

    #[derive(Debug, Default, Clone, Copy)]
    pub struct Utf16Be;

    impl Decoder for Utf16Be {
        fn decodable(bytes: &str) -> bool {
            bytes.is_empty()
        }

        fn decode(self, bytes: &[u8]) -> Result<Cow<'_, str>, UnknownEncoding> {
            use encoding_rs::UTF_16BE;

            Ok(decode_as(bytes, UTF_16BE, Bom::from(bytes)))
        }
    }

    #[derive(Debug, Default, Clone, Copy)]
    pub struct Utf16Le;

    impl Decoder for Utf16Le {
        fn decodable(bytes: &str) -> bool {
            bytes.is_empty()
        }

        fn decode(self, bytes: &[u8]) -> Result<Cow<'_, str>, UnknownEncoding> {
            use encoding_rs::UTF_16LE;

            Ok(decode_as(bytes, UTF_16LE, Bom::from(bytes)))
        }
    }

    #[derive(Debug, Default, Clone, Copy)]
    pub struct ShiftJis;

    impl Decoder for ShiftJis {
        fn decodable(bytes: &str) -> bool {
            bytes.is_ascii()
        }

        fn decode(self, bytes: &[u8]) -> Result<Cow<'_, str>, UnknownEncoding> {
            use encoding_rs::SHIFT_JIS;

            Ok(SHIFT_JIS.decode_without_bom_handling(bytes).0)
        }
    }

    #[derive(Debug, Default, Clone, Copy)]
    pub struct EucJp;

    impl Decoder for EucJp {
        fn decodable(bytes: &str) -> bool {
            bytes.is_ascii()
        }

        fn decode(self, bytes: &[u8]) -> Result<Cow<'_, str>, UnknownEncoding> {
            use encoding_rs::EUC_JP;

            Ok(EUC_JP.decode_without_bom_handling(bytes).0)
        }
    }

    #[derive(Debug, Default, Clone, Copy)]
    pub struct Dynamic;

    impl Decoder for Dynamic {
        fn decodable(_bytes: &str) -> bool {
            true
        }

        fn decode(self, bytes: &[u8]) -> Result<Cow<'_, str>, UnknownEncoding> {
            use encoding_rs::{DecoderResult, EUC_JP, SHIFT_JIS, UTF_16BE, UTF_16LE};

            use std::str::from_utf8;

            if let Ok(s) = from_utf8(bytes) {
                return Ok(Cow::Borrowed(s));
            }

            let bom = Bom::from(bytes);

            let mut states = [SHIFT_JIS, EUC_JP, UTF_16BE, UTF_16LE]
                .map(|enc| (enc, enc.new_decoder_without_bom_handling(), true, bytes));
            let mut buf = [0; 512];

            loop {
                for (enc, dec, valid, remaining) in &mut states {
                    if !*valid {
                        continue;
                    }
                    let (res, read, _) =
                        dec.decode_to_utf8_without_replacement(remaining, &mut buf, true);
                    match res {
                        DecoderResult::InputEmpty => {
                            return Ok(decode_as(bytes, enc, bom));
                        }
                        DecoderResult::Malformed(..) => *valid = false,
                        DecoderResult::OutputFull => *remaining = &remaining[read..],
                    }
                }

                if states.iter().all(|(_, _, valid, _)| !valid) {
                    return Err(UnknownEncoding);
                }
            }
        }
    }

    fn decode_as<'a>(bytes: &'a [u8], enc: &'static RsEncoding, bom: Bom) -> Cow<'a, str> {
        use encoding_rs::{UTF_16BE, UTF_16LE};

        let has_bom =
            (enc == UTF_16BE && bom == Bom::Utf16Be) || (enc == UTF_16LE && bom == Bom::Utf16Le);
        if has_bom {
            enc.decode_with_bom_removal(bytes).0
        } else {
            enc.decode_without_bom_handling(bytes).0
        }
    }
}

#[cfg(any(feature = "txt-sync", feature = "csv-sync"))]
pub(crate) fn decode_owned<D: Decoder>(
    bytes: Vec<u8>,
    decoder: D,
) -> Result<String, UnknownEncoding> {
    decoder.decode(&bytes).map(|s| s.into_owned())
}

#[cfg(any(feature = "txt-sync", feature = "csv-sync"))]
pub(crate) fn decode_cow<D: Decoder>(
    bytes: Cow<'_, [u8]>,
    decoder: D,
) -> Result<Cow<'_, str>, UnknownEncoding> {
    match bytes {
        Cow::Borrowed(bytes) => decoder.decode(bytes),
        Cow::Owned(bytes) => decode_owned(bytes, decoder).map(Cow::Owned),
    }
}

#[cfg(all(test, feature = "decode"))]
mod tests {
    use super::{Decoder as _, Dynamic, EucJp, ShiftJis, Utf8, Utf16Be, Utf16Le};

    #[test]
    fn checks_decodable_charsets() {
        use std::{array::from_fn, str::from_utf8};

        let bytes: [u8; 128] = from_fn(|i| i as u8);
        let bytes = from_utf8(&bytes).unwrap();
        assert!(Utf8::decodable(bytes));
        assert!(Dynamic::decodable(bytes));
        assert!(ShiftJis::decodable(bytes));
        assert!(EucJp::decodable(bytes));
        assert!(!Utf16Be::decodable(bytes));
        assert!(!Utf16Le::decodable(bytes));

        for bytes in [
            "春はあけぼの。",
            "é",
            "aé",
            "🌸",
            "\u{80}",
            "\u{fffd}",
            "\u{feff}",
        ] {
            assert!(Utf8::decodable(bytes));
            assert!(Dynamic::decodable(bytes));
            assert!(!ShiftJis::decodable(bytes));
            assert!(!EucJp::decodable(bytes));
            assert!(!Utf16Be::decodable(bytes));
            assert!(!Utf16Le::decodable(bytes));
        }
    }

    #[test]
    fn accepts_empty_utf8_input() {
        assert!(Utf8::decodable(""));
        assert!(Dynamic::decodable(""));
        assert!(ShiftJis::decodable(""));
        assert!(EucJp::decodable(""));
        assert!(Utf16Be::decodable(""));
        assert!(Utf16Le::decodable(""));
    }

    #[test]
    fn decodes_supported_charsets() {
        let utf8_bytes = "春はあけぼの。";
        assert!(
            Utf8.decode(utf8_bytes.as_bytes())
                .is_ok_and(|s| s == utf8_bytes)
        );

        let shift_jis_bytes = &[
            0x8F, 0x74, 0x82, 0xCD, 0x82, 0xA0, 0x82, 0xAF, 0x82, 0xDA, 0x82, 0xCC, 0x81, 0x42,
        ];
        assert!(
            ShiftJis
                .decode(shift_jis_bytes)
                .is_ok_and(|s| s == utf8_bytes)
        );

        let euc_jp_bytes = &[
            0xBD, 0xD5, 0xA4, 0xCF, 0xA4, 0xA2, 0xA4, 0xB1, 0xA4, 0xDC, 0xA4, 0xCE, 0xA1, 0xA3,
        ];
        assert!(EucJp.decode(euc_jp_bytes).is_ok_and(|s| s == utf8_bytes));

        let utf16_be_bytes = &[
            0x66, 0x25, 0x30, 0x6F, 0x30, 0x42, 0x30, 0x51, 0x30, 0x7C, 0x30, 0x6E, 0x30, 0x02,
        ];
        assert!(
            Utf16Be
                .decode(utf16_be_bytes)
                .is_ok_and(|s| s == utf8_bytes)
        );

        let utf16_be_bytes_bom = &[
            0xFE, 0xFF, 0x66, 0x25, 0x30, 0x6F, 0x30, 0x42, 0x30, 0x51, 0x30, 0x7C, 0x30, 0x6E,
            0x30, 0x02,
        ];
        assert!(
            Utf16Be
                .decode(utf16_be_bytes_bom)
                .is_ok_and(|s| s == utf8_bytes)
        );

        let utf16_le_bytes = &[
            0x25, 0x66, 0x6F, 0x30, 0x42, 0x30, 0x51, 0x30, 0x7C, 0x30, 0x6E, 0x30, 0x02, 0x30,
        ];
        assert!(
            Utf16Le
                .decode(utf16_le_bytes)
                .is_ok_and(|s| s == utf8_bytes)
        );

        let utf16_le_bytes_bom = &[
            0xFF, 0xFE, 0x25, 0x66, 0x6F, 0x30, 0x42, 0x30, 0x51, 0x30, 0x7C, 0x30, 0x6E, 0x30,
            0x02, 0x30,
        ];
        assert!(
            Utf16Le
                .decode(utf16_le_bytes_bom)
                .is_ok_and(|s| s == utf8_bytes)
        );

        assert!(
            Dynamic
                .decode(utf8_bytes.as_bytes())
                .is_ok_and(|s| s == utf8_bytes)
        );
        assert!(
            Dynamic
                .decode(shift_jis_bytes)
                .is_ok_and(|s| s == "春はあけぼの。")
        );
        assert!(
            ShiftJis
                .decode(euc_jp_bytes)
                .is_ok_and(|s| s == "ｽﾕ､ﾏ､｢､ｱ､ﾜ､ﾎ｡｣")
        );
        assert!(
            Dynamic
                .decode(euc_jp_bytes)
                .is_ok_and(|s| s == "ｽﾕ､ﾏ､｢､ｱ､ﾜ､ﾎ｡｣")
        );
    }

    #[test]
    fn removes_matching_boms() {
        assert!(
            Utf8.decode(&[0xEF, 0xBB, 0xBF, 0x61])
                .is_ok_and(|s| s == "a")
        );
        assert!(
            Utf16Be
                .decode(&[0xFE, 0xFF, 0x00, 0x61])
                .is_ok_and(|s| s == "a")
        );
        assert!(
            Utf16Le
                .decode(&[0xFF, 0xFE, 0x61, 0x00])
                .is_ok_and(|s| s == "a")
        );
    }
}
