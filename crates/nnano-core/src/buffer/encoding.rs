//! Encoding: detect on load, never transcode on save (phase 4).
//!
//! Two encodings and no negotiation:
//! - **UTF-8** when the bytes parse as UTF-8 (also covers pure ASCII);
//! - **Latin-1** otherwise: byte `0xNN` becomes `U+00NN` and back, a bijection, so
//!   an untouched Latin-1 file is written back byte-identical.
//!
//! UTF-16 is refused instead of guessed (decided: unsupported). A UTF-8 BOM is
//! left in the text as `U+FEFF`, which is zero-width and round-trips.
//!
//! Nothing here re-encodes line breaks or drops bytes: saving writes each chunk as
//! it is, so `\r\n`, a lone `\r` and any other kept byte survive a save.

use std::fmt;
use std::io::{self, Write};

/// What the file was detected as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Latin1,
}

/// Decoded text plus what it was read as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded {
    pub text: String,
    pub encoding: Encoding,
}

#[derive(Debug)]
pub enum DecodeError {
    /// A UTF-16 BOM: refused, never guessed.
    Utf16,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Utf16 => write!(f, "UTF-16 is not supported"),
        }
    }
}

impl std::error::Error for DecodeError {}

#[derive(Debug)]
pub enum EncodeError {
    /// A char outside `U+0000..=U+00FF` cannot be written as Latin-1.
    NotRepresentable(char),
    Io(io::Error),
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRepresentable(ch) => {
                write!(f, "character {ch:?} is not representable in Latin-1")
            }
            Self::Io(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for EncodeError {}

/// Detect and decode. Never fails on odd bytes: anything that is not UTF-8 is
/// read as Latin-1, which is total.
pub fn decode(bytes: &[u8]) -> Result<Loaded, DecodeError> {
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        return Err(DecodeError::Utf16);
    }
    Ok(match std::str::from_utf8(bytes) {
        Ok(text) => Loaded {
            text: text.to_string(),
            encoding: Encoding::Utf8,
        },
        Err(_) => Loaded {
            text: bytes.iter().map(|byte| char::from(*byte)).collect(),
            encoding: Encoding::Latin1,
        },
    })
}

/// Encode `chunks` into `writer` without building a copy of the whole buffer.
/// Stops at the first char Latin-1 cannot hold instead of replacing it.
pub fn write_encoded<'a, W: Write>(
    writer: &mut W,
    chunks: impl IntoIterator<Item = &'a str>,
    encoding: Encoding,
) -> Result<(), EncodeError> {
    match encoding {
        Encoding::Utf8 => {
            for chunk in chunks {
                writer
                    .write_all(chunk.as_bytes())
                    .map_err(EncodeError::Io)?;
            }
        }
        Encoding::Latin1 => {
            let mut buffer = Vec::with_capacity(8192);
            for chunk in chunks {
                for ch in chunk.chars() {
                    match latin1_byte(ch) {
                        Some(byte) => buffer.push(byte),
                        None => return Err(EncodeError::NotRepresentable(ch)),
                    }
                    if buffer.len() >= 8192 {
                        writer.write_all(&buffer).map_err(EncodeError::Io)?;
                        buffer.clear();
                    }
                }
            }
            if !buffer.is_empty() {
                writer.write_all(&buffer).map_err(EncodeError::Io)?;
            }
        }
    }
    Ok(())
}

/// Convenience for tests and small strings.
pub fn encode(text: &str, encoding: Encoding) -> Result<Vec<u8>, EncodeError> {
    let mut out = Vec::with_capacity(text.len());
    write_encoded(&mut out, std::iter::once(text), encoding)?;
    Ok(out)
}

fn latin1_byte(ch: char) -> Option<u8> {
    u8::try_from(u32::from(ch)).ok()
}

#[cfg(test)]
mod tests {
    use super::{DecodeError, EncodeError, Encoding, decode, encode};

    #[test]
    fn ascii_is_utf8_and_identical_in_both() {
        let loaded = decode(b"hola\n").unwrap();
        assert_eq!(loaded.encoding, Encoding::Utf8);
        assert_eq!(encode(&loaded.text, Encoding::Latin1).unwrap(), b"hola\n");
    }

    #[test]
    fn utf8_accents_and_cjk_round_trip_byte_identically() {
        let bytes = "café 漢字\r\n".as_bytes();
        let loaded = decode(bytes).unwrap();
        assert_eq!(loaded.encoding, Encoding::Utf8);
        assert_eq!(encode(&loaded.text, Encoding::Utf8).unwrap(), bytes);
    }

    #[test]
    fn latin1_bytes_round_trip_byte_identically() {
        // 0xE9 is 'é' in Latin-1 and invalid UTF-8 on its own.
        let bytes = b"caf\xE9\r\n\xFF";
        let loaded = decode(bytes).unwrap();
        assert_eq!(loaded.encoding, Encoding::Latin1);
        assert!(loaded.text.starts_with("café"));
        assert_eq!(encode(&loaded.text, loaded.encoding).unwrap(), bytes);
    }

    #[test]
    fn c1_bytes_survive_even_if_they_are_invisible() {
        let bytes = [0x80u8, 0x9F];
        let loaded = decode(&bytes).unwrap();
        assert_eq!(loaded.encoding, Encoding::Latin1);
        assert_eq!(encode(&loaded.text, Encoding::Latin1).unwrap(), bytes);
    }

    #[test]
    fn utf16_bom_is_refused_instead_of_guessed() {
        assert!(matches!(
            decode(&[0xFF, 0xFE, b'h', 0]),
            Err(DecodeError::Utf16)
        ));
        assert!(matches!(
            decode(&[0xFE, 0xFF, 0, b'h']),
            Err(DecodeError::Utf16)
        ));
    }

    #[test]
    fn utf8_bom_is_kept_so_it_round_trips() {
        let bytes = b"\xEF\xBB\xBFhola";
        let loaded = decode(bytes).unwrap();
        assert!(loaded.text.starts_with('\u{FEFF}'));
        assert_eq!(encode(&loaded.text, Encoding::Utf8).unwrap(), bytes);
    }

    #[test]
    fn a_char_latin1_cannot_hold_is_an_error_not_a_replacement() {
        let err = encode("漢", Encoding::Latin1).unwrap_err();
        assert!(matches!(err, EncodeError::NotRepresentable('漢')));
        // 'ÿ' is U+00FF and still fits.
        assert_eq!(encode("ÿ", Encoding::Latin1).unwrap(), [0xFF]);
    }
}
