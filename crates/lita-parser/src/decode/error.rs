use std::error::Error;
use std::fmt;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct UnknownEncoding;

impl fmt::Display for UnknownEncoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unknown character encoding")
    }
}

impl Error for UnknownEncoding {}
