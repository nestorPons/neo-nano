//! File I/O the app loop delegates: the in-place save, path completion and the
//! absolute form of a typed path.
//!
//! It lives in `nnano-core` (not in the TUI) so the truncate-write-flush-sync
//! order, the Latin-1 refusal and the completion rules are testable without a
//! TTY. The app loop keeps owning *which* buffer is saved and *what* is reported.

use std::fs::{self, OpenOptions};
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::buffer::{EncodeError, Encoding, TextBuffer, encoding};

/// Write `buffer` over `path` **in place**: truncate the real file, stream the
/// chunks, flush, then `fsync`.
///
/// No temp file, no `rename`, no `file~`: hardlinks and symlinks survive, and the
/// inode other processes watch does not change. The cost is accepted and
/// documented — an interrupted write leaves a truncated file, and the recovery
/// file is the only net. `sync_all` is the second half of that promise: without
/// it the bytes are only in the page cache when `^S` returns.
pub fn write_in_place(path: &Path, buffer: &TextBuffer, encoding: Encoding) -> io::Result<()> {
    // Opening truncates, so a char Latin-1 cannot hold would destroy the file
    // before the encoder refused it. The whole buffer is checked first: one extra
    // O(n) pass only for Latin-1, which is the only way to keep the in-place
    // promise without writing a temp copy.
    if encoding == Encoding::Latin1 {
        for chunk in buffer.chunks() {
            if let Some(ch) = chunk.chars().find(|ch| u32::from(*ch) > 0xFF) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    EncodeError::NotRepresentable(ch),
                ));
            }
        }
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    match encoding::write_encoded(&mut file, buffer.chunks(), encoding) {
        Ok(()) => {}
        Err(EncodeError::Io(err)) => return Err(err),
        Err(err @ EncodeError::NotRepresentable(_)) => {
            return Err(io::Error::new(io::ErrorKind::InvalidData, err));
        }
    }
    file.flush()?;
    file.sync_all()
}

/// Absolute form of a typed path, relative to the process working directory.
/// Fails only when the working directory is gone.
pub fn absolute(path: &str) -> io::Result<PathBuf> {
    std::path::absolute(Path::new(path))
}

/// Directory entries that complete `input`, sorted, with `/` appended to
/// directories. An empty vector means "no completion", never an error: a prompt
/// must not complain mid-typing.
///
/// The rule is one path component deep, as a shell does for a single Tab: the text
/// up to the last `/` is the directory, the rest is the prefix. Dotfiles only
/// complete when the prefix starts with a dot.
pub fn complete(input: &str) -> Vec<String> {
    let (dir, prefix) = match input.rfind('/') {
        Some(at) => (&input[..=at], &input[at + 1..]),
        None => ("", input),
    };
    let read = if dir.is_empty() { "." } else { dir };
    let Ok(entries) = fs::read_dir(read) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(prefix) {
            continue;
        }
        if name.starts_with('.') && !prefix.starts_with('.') {
            continue;
        }
        let mut candidate = format!("{dir}{name}");
        if entry.path().is_dir() {
            candidate.push('/');
        }
        out.push(candidate);
    }
    out.sort();
    out
}

/// Longest common prefix of `values`, char-wise. Used to extend a Tab completion
/// when more than one candidate matches.
pub fn common_prefix(values: &[String]) -> String {
    let Some(first) = values.first() else {
        return String::new();
    };
    let mut end = first.chars().count();
    for value in &values[1..] {
        let shared = first
            .chars()
            .zip(value.chars())
            .take_while(|(a, b)| a == b)
            .count();
        end = end.min(shared);
    }
    first.chars().take(end).collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{absolute, common_prefix, complete, write_in_place};
    use crate::buffer::{Encoding, TextBuffer};

    fn unique_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nnano-file-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn an_in_place_save_truncates_and_keeps_the_bytes_it_was_given() {
        let dir = unique_dir("save");
        let path = dir.join("notes.txt");
        std::fs::write(&path, "mucho más texto del que queda").unwrap();

        let buffer = TextBuffer::from_text("uno\r\ndos\r\n");
        write_in_place(&path, &buffer, Encoding::Utf8).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"uno\r\ndos\r\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_in_place_save_keeps_the_inode() {
        let dir = unique_dir("inode");
        let path = dir.join("same.txt");
        std::fs::write(&path, "x").unwrap();
        let before = std::fs::metadata(&path).unwrap();
        write_in_place(&path, &TextBuffer::from_text("y"), Encoding::Utf8).unwrap();
        let after = std::fs::metadata(&path).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(before.ino(), after.ino(), "no rename, no new inode");
            assert_eq!(after.nlink(), 1);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn writing_latin1_refuses_a_char_it_cannot_hold_before_opening() {
        let dir = unique_dir("latin1");
        let path = dir.join("latin1.txt");
        std::fs::write(&path, b"caf\xE9\n").unwrap();
        let buffer = TextBuffer::from_text("漢");
        let err = write_in_place(&path, &buffer, Encoding::Latin1).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        // The original bytes are still there: the refusal is on representation,
        // not a half-written file.
        assert_eq!(std::fs::read(&path).unwrap(), b"caf\xE9\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn absolute_resolves_against_the_working_directory() {
        let absolute = absolute("notes.txt").unwrap();
        assert!(absolute.is_absolute());
        assert!(absolute.ends_with("notes.txt"));
    }

    #[test]
    fn completion_lists_matching_entries_and_marks_directories() {
        let dir = unique_dir("complete");
        std::fs::write(dir.join("alpha.txt"), "").unwrap();
        std::fs::write(dir.join("alpine.txt"), "").unwrap();
        std::fs::write(dir.join("beta.txt"), "").unwrap();
        std::fs::create_dir_all(dir.join("album")).unwrap();
        std::fs::write(dir.join(".hidden"), "").unwrap();

        let input = format!("{}/al", dir.display());
        let found = complete(&input);
        let names: Vec<String> = found
            .iter()
            .map(|path| {
                path.strip_prefix(&format!("{}/", dir.display()))
                    .unwrap_or(path)
                    .to_string()
            })
            .collect();
        assert_eq!(
            names,
            vec!["album/", "alpha.txt", "alpine.txt"],
            "sorted, directories marked, dotfiles skipped"
        );

        // A dot prefix reaches the dotfile.
        let input = format!("{}/.h", dir.display());
        assert_eq!(complete(&input).len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_bare_component_is_read_from_the_working_directory() {
        // No slash: the prefix is read from ".", which always exists. The test
        // calls it with a name no checkout has, so the result is empty rather
        // than an error (the cwd is never mutated: tests run in parallel).
        assert!(complete("nnano-no-such-entry-").is_empty());
    }

    #[test]
    fn no_completion_is_an_empty_list_not_an_error() {
        assert!(complete("/definitely/not/here/x").is_empty());
        let _ = complete("");
    }

    #[test]
    fn the_common_prefix_stops_at_the_first_difference() {
        let values = vec!["alpha".to_string(), "alpine".to_string()];
        assert_eq!(common_prefix(&values), "alp");
        assert_eq!(common_prefix(&["solo".to_string()]), "solo");
        assert_eq!(common_prefix(&[]), "");
    }
}
