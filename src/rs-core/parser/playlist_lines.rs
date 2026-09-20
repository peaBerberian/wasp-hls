use std::{
    borrow::Cow,
    io::{self, BufRead},
};

/// Read one line, preserving malformed UTF-8 with replacement characters.
/// The caller can reuse `bytes` for every line.
pub(super) fn read_playlist_line<'a>(
    reader: &mut impl BufRead,
    bytes: &'a mut Vec<u8>,
) -> io::Result<Option<Cow<'a, str>>> {
    bytes.clear();
    if reader.read_until(b'\n', bytes)? == 0 {
        return Ok(None);
    }
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    Ok(Some(String::from_utf8_lossy(bytes)))
}
