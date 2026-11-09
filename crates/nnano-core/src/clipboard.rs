//! Copying out to the system clipboard with OSC 52 (opt-in, capped).
//!
//! The internal register is always the storage; OSC 52 is a *copy out* that works
//! over SSH with no X server. It is off by default because some terminals ignore
//! it and a slow tty can stall on a big sequence, so a copy is capped at
//! [`OSC52_LIMIT`] bytes and truncation is reported instead of blocking.
//!
//! Base64 is written here rather than pulled in: it is 20 lines and one less
//! dependency against the binary-size budget.

use std::io;

/// Largest text sent per copy. Above it the sequence is truncated at a char
/// boundary and the caller warns.
pub const OSC52_LIMIT: usize = 64 * 1024;

/// Truncate `text` to at most `limit` bytes on a char boundary. Returns the kept
/// slice and whether anything was dropped.
pub fn capped(text: &str, limit: usize) -> (&str, bool) {
    if text.len() <= limit {
        return (text, false);
    }
    let mut end = limit;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    (&text[..end], true)
}

/// Standard base64 with padding, the alphabet OSC 52 uses.
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = u32::from(*chunk.get(1).unwrap_or(&0));
        let b2 = u32::from(*chunk.get(2).unwrap_or(&0));
        let block = (b0 << 16) | (b1 << 8) | b2;
        out.push(char::from(ALPHABET[(block >> 18) as usize & 63]));
        out.push(char::from(ALPHABET[(block >> 12) as usize & 63]));
        out.push(if chunk.len() > 1 {
            char::from(ALPHABET[(block >> 6) as usize & 63])
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            char::from(ALPHABET[block as usize & 63])
        } else {
            '='
        });
    }
    out
}

/// The whole escape sequence for one copy, capped. `true` means it was truncated.
pub fn sequence(text: &str) -> (String, bool) {
    let (kept, truncated) = capped(text, OSC52_LIMIT);
    (
        format!("\x1b]52;c;{}\x07", base64(kept.as_bytes())),
        truncated,
    )
}

/// Write an already built sequence and flush it: the app loop is between frames,
/// so nothing else would push it out.
pub fn emit(sequence: &str) -> io::Result<()> {
    use std::io::Write;
    let mut out = io::stdout();
    out.write_all(sequence.as_bytes())?;
    out.flush()
}

#[cfg(test)]
mod tests {
    use super::{OSC52_LIMIT, base64, capped, sequence};

    #[test]
    fn base64_matches_the_reference_vectors() {
        // RFC 4648, section 10.
        for (plain, encoded) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(plain.as_bytes()), encoded, "{plain:?}");
        }
    }

    #[test]
    fn base64_handles_multibyte_and_binary_bytes() {
        assert_eq!(base64("ñ".as_bytes()), "w7E=");
        assert_eq!(base64(&[0xFF, 0x00, 0x80]), "/wCA");
    }

    #[test]
    fn a_short_copy_is_untouched() {
        let (kept, truncated) = capped("hola", OSC52_LIMIT);
        assert_eq!(kept, "hola");
        assert!(!truncated);
    }

    #[test]
    fn a_long_copy_is_cut_on_a_char_boundary() {
        // 'ñ' is two bytes, so a limit of 3 must not split it.
        let text = "añañañ";
        let (kept, truncated) = capped(text, 3);
        assert!(truncated);
        assert_eq!(kept, "añ");
        assert!(kept.len() <= 3);
        assert!(text.starts_with(kept));
    }

    #[test]
    fn the_sequence_is_osc52_with_base64_and_st() {
        let (sequence, truncated) = sequence("hi");
        assert_eq!(sequence, "\x1b]52;c;aGk=\x07");
        assert!(!truncated);
    }

    #[test]
    fn the_sequence_reports_the_cap() {
        let text = "x".repeat(OSC52_LIMIT + 10);
        let (sequence, truncated) = sequence(&text);
        assert!(truncated);
        assert!(sequence.starts_with("\x1b]52;c;"));
        assert!(sequence.ends_with('\x07'));
        // 64 KiB of text is 87384 bytes of base64 plus the wrapper.
        assert_eq!(sequence.len(), 8 + 87384);
    }
}
