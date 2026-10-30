use encoding_rs::Encoding;

use std::borrow::Cow;
use std::fmt;

mod acceptable_encodings {
    pub(super) use encoding_rs::{EUC_JP, SHIFT_JIS, UTF_16BE, UTF_16LE};
}

/// Represents the character encodings.
///
/// When the feature flag `decode` is disabled, `Charset` only accepts `Charset::Utf8` and it is the
/// [`Default`] value.
///
/// When the feature flag is enabled, we support more encodings but still `Charset::Utf8` is the default.
///
/// When the `Charset` is set to `Dynamic`, the character encoding is determined in the following order:
///
/// - UTF-8
/// - SHIFT-JIS
/// - EUC-JP
/// - UTF-16 BE
/// - UTF-16 LE
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Charset {
    #[default]
    Utf8,
    Utf16Be,
    Utf16Le,
    ShiftJis,
    EucJp,
    Dynamic,
}

impl Charset {
    fn get_encoding(self) -> &'static Encoding {
        use acceptable_encodings::*;

        match self {
            Self::Utf16Be => UTF_16BE,
            Self::Utf16Le => UTF_16LE,
            Self::ShiftJis => SHIFT_JIS,
            Self::EucJp => EUC_JP,
            _ => unreachable!(),
        }
    }

    fn list_encodings() -> [(Self, &'static Encoding); 4] {
        use acceptable_encodings::*;

        [
            (Self::ShiftJis, SHIFT_JIS),
            (Self::EucJp, EUC_JP),
            (Self::Utf16Be, UTF_16BE),
            (Self::Utf16Le, UTF_16LE),
        ]
    }
}

#[derive(Debug, Clone)]
pub struct UnknownEncoding;

impl fmt::Display for UnknownEncoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown character encoding (acceptable encodings: UTF-8, UTF-16 BE/LE, SHIFT-JIS, EUC-JP)"
        )
    }
}
impl std::error::Error for UnknownEncoding {}

fn decode_as(bytes: &[u8], charset: Charset, has_bom: bool) -> Cow<'_, str> {
    match charset {
        Charset::Utf8 => {
            let bytes = if has_bom { &bytes[3..] } else { bytes };
            String::from_utf8_lossy(bytes)
        }
        Charset::Dynamic => unreachable!(),
        _ => {
            let encoding = charset.get_encoding();
            if has_bom {
                let (res, _) = encoding.decode_with_bom_removal(bytes);
                res
            } else {
                let (res, _) = encoding.decode_without_bom_handling(bytes);
                res
            }
        }
    }
}

pub(crate) fn decode(bytes: &[u8], charset: Charset) -> Result<Cow<'_, str>, UnknownEncoding> {
    use unicode_bom::Bom;
    let bom = Bom::from(bytes);

    let has_bom = match (bom, charset) {
        (Bom::Null, _) => false,
        (Bom::Utf8, Charset::Utf8)
        | (Bom::Utf16Be, Charset::Utf16Be)
        | (Bom::Utf16Le, Charset::Utf16Le) => true,
        _ => false,
    };

    if charset != Charset::Dynamic {
        return Ok(decode_as(bytes, charset, has_bom));
    }

    if let Ok(s) = std::str::from_utf8(bytes) {
        let s = if has_bom { &s[3..] } else { s };
        return Ok(Cow::Borrowed(s));
    }

    let mut states = Charset::list_encodings().map(|(charset, encoding)| {
        (
            charset,
            encoding.new_decoder_without_bom_handling(),
            true,
            bytes,
        )
    });

    let mut buf = [0u8; 512];

    let detected: Charset;

    'a: loop {
        use encoding_rs::DecoderResult;

        for (charset, decoder, valid, remaining) in states.iter_mut() {
            if !*valid {
                continue;
            }

            let (result, read, _) =
                decoder.decode_to_utf8_without_replacement(remaining, &mut buf, true);

            match result {
                DecoderResult::InputEmpty => {
                    detected = *charset;
                    break 'a;
                }
                DecoderResult::Malformed(..) => *valid = false,
                _ => *remaining = &remaining[read..],
            }
        }

        if states.iter().all(|(_, _, valid, _)| !valid) {
            detected = Charset::Utf8;
            break;
        }
    }

    if detected == Charset::Utf8 {
        Err(UnknownEncoding)
    } else {
        let has_bom = (detected == Charset::Utf16Be && bom == Bom::Utf16Be)
            || (detected == Charset::Utf16Le && bom == Bom::Utf16Le);
        Ok(decode_as(bytes, detected, has_bom))
    }
}

#[cfg(test)]
mod tests {
    use super::{Charset, decode};

    #[test]
    fn decodes_supported_charsets() {
        let utf8_bytes = "春はあけぼの。";
        assert!(decode(utf8_bytes.as_bytes(), Charset::Utf8).is_ok_and(|s| s == utf8_bytes));

        let shift_jis_bytes = &[
            0x8F, 0x74, 0x82, 0xCD, 0x82, 0xA0, 0x82, 0xAF, 0x82, 0xDA, 0x82, 0xCC, 0x81, 0x42,
        ];
        assert!(decode(shift_jis_bytes, Charset::ShiftJis).is_ok_and(|s| s == utf8_bytes));

        let euc_jp_bytes = &[
            0xBD, 0xD5, 0xA4, 0xCF, 0xA4, 0xA2, 0xA4, 0xB1, 0xA4, 0xDC, 0xA4, 0xCE, 0xA1, 0xA3,
        ];
        assert!(decode(euc_jp_bytes, Charset::EucJp).is_ok_and(|s| s == utf8_bytes));

        let utf16_be_bytes = &[
            0x66, 0x25, 0x30, 0x6F, 0x30, 0x42, 0x30, 0x51, 0x30, 0x7C, 0x30, 0x6E, 0x30, 0x02,
        ];
        assert!(decode(utf16_be_bytes, Charset::Utf16Be).is_ok_and(|s| s == utf8_bytes));

        let utf16_be_bytes_bom = &[
            0xFE, 0xFF, 0x66, 0x25, 0x30, 0x6F, 0x30, 0x42, 0x30, 0x51, 0x30, 0x7C, 0x30, 0x6E,
            0x30, 0x02,
        ];
        assert!(decode(utf16_be_bytes_bom, Charset::Utf16Be).is_ok_and(|s| s == utf8_bytes));

        let utf16_le_bytes = &[
            0x25, 0x66, 0x6F, 0x30, 0x42, 0x30, 0x51, 0x30, 0x7C, 0x30, 0x6E, 0x30, 0x02, 0x30,
        ];
        assert!(decode(utf16_le_bytes, Charset::Utf16Le).is_ok_and(|s| s == utf8_bytes));

        let utf16_le_bytes_bom = &[
            0xFF, 0xFE, 0x25, 0x66, 0x6F, 0x30, 0x42, 0x30, 0x51, 0x30, 0x7C, 0x30, 0x6E, 0x30,
            0x02, 0x30,
        ];
        assert!(decode(utf16_le_bytes_bom, Charset::Utf16Le).is_ok_and(|s| s == utf8_bytes));

        assert!(decode(utf8_bytes.as_bytes(), Charset::Dynamic).is_ok_and(|s| s == utf8_bytes));
        assert!(decode(shift_jis_bytes, Charset::Dynamic).is_ok_and(|s| s == "春はあけぼの。"));
        assert!(decode(euc_jp_bytes, Charset::ShiftJis).is_ok_and(|s| s == "ｽﾕ､ﾏ､｢､ｱ､ﾜ､ﾎ｡｣"));
        assert!(decode(euc_jp_bytes, Charset::Dynamic).is_ok_and(|s| s == "ｽﾕ､ﾏ､｢､ｱ､ﾜ､ﾎ｡｣"));
    }

    #[test]
    fn removes_matching_boms() {
        for (bytes, charset) in [
            (&[0xEF, 0xBB, 0xBF, 0x61][..], Charset::Utf8),
            (&[0xFE, 0xFF, 0x00, 0x61][..], Charset::Utf16Be),
            (&[0xFF, 0xFE, 0x61, 0x00][..], Charset::Utf16Le),
        ] {
            assert!(decode(bytes, charset).is_ok_and(|s| s == "a"));
        }
    }
}
