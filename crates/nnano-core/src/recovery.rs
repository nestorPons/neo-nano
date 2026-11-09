//! The recovery file: unsaved work survives a crash or a killed SSH session.
//!
//! `^S` writes in place, so an interrupted write is the one way to lose data; this
//! file is the net. It is not a save: it is written only when the buffer is dirty,
//! 2 s after the last edit, never from the redraw cycle.
//!
//! - Location: `<state dir>/recovery/<hash>.swp`, the hash taken over the
//!   **absolute** path. One directory, no index to keep in sync, so a snapshot is
//!   found from the file name alone. The hash is `DefaultHasher` (SipHash-1-3),
//!   stable within a build — a recovery file is a same-install artefact, not a
//!   portable format.
//! - Body: the text in the buffer's own encoding, so a Latin-1 file round-trips
//!   byte-identically and nothing is ever transcoded behind the user's back. A
//!   Latin-1 buffer holding a char it cannot encode falls back to UTF-8 and says
//!   so in the header.
//! - Replacement: written to `<name>.tmp` and renamed, so a crash mid-snapshot
//!   cannot leave a torn file that shadows the useful one.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::buffer::{Encoding, encoding};

/// Idle time after the last edit before a snapshot is written.
pub const DEBOUNCE: Duration = Duration::from_secs(2);

const MAGIC: &str = "nnano-recovery v1";

/// One buffer as it was, with the path it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub path: PathBuf,
    pub encoding: Encoding,
    pub text: String,
}

impl Snapshot {
    /// Header (UTF-8) + blank line + body (the buffer's encoding).
    pub fn encode(&self) -> io::Result<Vec<u8>> {
        // A Latin-1 buffer can hold a char Latin-1 cannot encode (it was typed in
        // this session). The snapshot must not fail because of that: UTF-8 is
        // lossless and the header records which one the body uses.
        let (encoding, body) = match encoding::encode(&self.text, self.encoding) {
            Ok(body) => (self.encoding, body),
            Err(_) => (
                Encoding::Utf8,
                encoding::encode(&self.text, Encoding::Utf8)
                    .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?,
            ),
        };
        let mut bytes = Vec::with_capacity(body.len() + 128);
        bytes.extend_from_slice(MAGIC.as_bytes());
        bytes.push(b'\n');
        bytes.extend_from_slice(b"path: ");
        bytes.extend_from_slice(self.path.to_string_lossy().as_bytes());
        bytes.push(b'\n');
        bytes.extend_from_slice(b"encoding: ");
        bytes.extend_from_slice(match encoding {
            Encoding::Utf8 => b"utf-8",
            Encoding::Latin1 => b"latin-1",
        });
        bytes.push(b'\n');
        bytes.push(b'\n');
        bytes.extend_from_slice(&body);
        Ok(bytes)
    }

    /// `None` for anything this module did not write (truncated, another tool,
    /// a future version): a bad snapshot is ignored, never a startup error.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let split = bytes.windows(2).position(|pair| pair == b"\n\n")?;
        let header = std::str::from_utf8(&bytes[..split]).ok()?;
        let body = &bytes[split + 2..];

        let mut lines = header.lines();
        if lines.next()? != MAGIC {
            return None;
        }
        let path = lines.next()?.strip_prefix("path: ")?;
        let encoding = match lines.next()?.strip_prefix("encoding: ")? {
            "utf-8" => Encoding::Utf8,
            "latin-1" => Encoding::Latin1,
            _ => return None,
        };
        let text = match encoding {
            Encoding::Utf8 => String::from_utf8(body.to_vec()).ok()?,
            Encoding::Latin1 => body.iter().map(|byte| char::from(*byte)).collect(),
        };
        Some(Self {
            path: PathBuf::from(path),
            encoding,
            text,
        })
    }
}

/// Snapshot path for `target` inside `dir`. Same target, same name; no target
/// shares a name with another (bar a 64-bit hash collision).
pub fn path_for(dir: &Path, target: &Path) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    target.hash(&mut hasher);
    dir.join(format!("{:016x}.swp", hasher.finish()))
}

/// Write the snapshot, creating `dir` when it is missing. Returns the path.
pub fn write(dir: &Path, snapshot: &Snapshot) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let path = path_for(dir, &snapshot.path);
    let temporary = path.with_extension("swp.tmp");
    fs::write(&temporary, snapshot.encode()?)?;
    fs::rename(&temporary, &path)?;
    Ok(path)
}

/// The snapshot for `target`, if there is one this module can read.
pub fn load(dir: &Path, target: &Path) -> Option<Snapshot> {
    let bytes = fs::read(path_for(dir, target)).ok()?;
    Snapshot::decode(&bytes)
}

/// Drop the snapshot for `target`. A missing file is success: callers delete after
/// a save and on exit, and neither may fail because there was nothing to delete.
pub fn remove(dir: &Path, target: &Path) {
    let _ = fs::remove_file(path_for(dir, target));
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{DEBOUNCE, Snapshot, load, path_for, remove, write};
    use crate::buffer::Encoding;

    fn unique_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("nnano-recovery-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn snapshot(path: &str, text: &str) -> Snapshot {
        Snapshot {
            path: PathBuf::from(path),
            encoding: Encoding::Utf8,
            text: text.to_string(),
        }
    }

    #[test]
    fn a_snapshot_round_trips_with_its_path_and_text() {
        let snapshot = snapshot("/home/ada/notes.txt", "uno\ndos\r\n");
        let bytes = snapshot.encode().unwrap();
        assert_eq!(Snapshot::decode(&bytes), Some(snapshot));
    }

    #[test]
    fn the_debounce_is_two_seconds() {
        assert_eq!(DEBOUNCE, std::time::Duration::from_secs(2));
    }

    #[test]
    fn a_latin1_snapshot_round_trips_byte_for_byte() {
        let snapshot = Snapshot {
            path: PathBuf::from("/tmp/latin1.txt"),
            encoding: Encoding::Latin1,
            text: "café\n".to_string(),
        };
        let bytes = snapshot.encode().unwrap();
        assert!(bytes.windows(9).any(|w| w == b"latin-1\n\n"));
        assert_eq!(Snapshot::decode(&bytes), Some(snapshot));
    }

    #[test]
    fn a_latin1_snapshot_with_a_char_it_cannot_hold_falls_back_to_utf8() {
        let snapshot = Snapshot {
            path: PathBuf::from("/tmp/mixed.txt"),
            encoding: Encoding::Latin1,
            text: "café 漢".to_string(),
        };
        let bytes = snapshot.encode().unwrap();
        let decoded = Snapshot::decode(&bytes).unwrap();
        assert_eq!(
            decoded.encoding,
            Encoding::Utf8,
            "header declares the truth"
        );
        assert_eq!(decoded.text, "café 漢");
    }

    #[test]
    fn garbage_and_truncated_files_decode_to_nothing() {
        assert!(Snapshot::decode(b"").is_none());
        assert!(Snapshot::decode(b"not a snapshot").is_none());
        assert!(Snapshot::decode(b"nnano-recovery v1\npath: /x\nencoding: utf-8\n").is_none());
        assert!(
            Snapshot::decode(b"nnano-recovery v1\npath: /x\nencoding: klingon\n\nbody").is_none()
        );
        // A UTF-8 header claiming Latin-1 is still readable as a bijection, but a
        // header claiming UTF-8 with invalid UTF-8 is not.
        assert!(
            Snapshot::decode(b"nnano-recovery v1\npath: /x\nencoding: utf-8\n\n\xFF").is_none()
        );
    }

    #[test]
    fn one_target_gets_one_file_and_two_targets_do_not_collide() {
        let dir = Path::new("/tmp/nnano-recovery-names");
        let first = path_for(dir, Path::new("/home/ada/a.txt"));
        let again = path_for(dir, Path::new("/home/ada/a.txt"));
        let other = path_for(dir, Path::new("/home/ada/b.txt"));
        assert_eq!(first, again);
        assert_ne!(first, other);
        assert_eq!(first.parent(), Some(dir));
        assert_eq!(first.extension().unwrap(), "swp");
    }

    #[test]
    fn writing_reading_and_removing_a_snapshot() {
        let dir = unique_dir("roundtrip");
        let target = dir.join("doc.txt");
        let snapshot = snapshot(target.to_str().unwrap(), "texto");

        let path = write(&dir.join("recovery"), &snapshot).unwrap();
        assert!(path.exists());
        assert_eq!(load(&dir.join("recovery"), &target), Some(snapshot));

        remove(&dir.join("recovery"), &target);
        assert!(!path.exists());
        assert_eq!(load(&dir.join("recovery"), &target), None);
        // Removing twice is not an error.
        remove(&dir.join("recovery"), &target);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn writing_creates_the_directory_and_leaves_no_temp_file() {
        let dir = unique_dir("mkdir");
        let target = dir.join("doc.txt");
        let recovery = dir.join("state/recovery");
        let path = write(&recovery, &snapshot(target.to_str().unwrap(), "x")).unwrap();
        assert!(path.exists());
        let leftovers: Vec<_> = std::fs::read_dir(&recovery)
            .unwrap()
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
