//! A logical byte stream whose pieces still belong to the input segment.
//!
//! TS headers interrupt the encoded data every 188 bytes. Joining those pieces
//! into a Vec would copy nearly the whole segment. Instead, keep their input
//! ranges here. Parsers use logical offsets; only final MP4 assembly copies the
//! payload. No range survives the synchronous transmux call.

use std::ops::Range;

use super::{Error, Result};

#[derive(Debug, Clone)]
struct Piece {
    logical_start: usize,
    input: Range<usize>,
}

#[derive(Debug, Default)]
pub(super) struct Stream {
    pieces: Vec<Piece>,
    pub len: usize,
}

impl Stream {
    pub fn push(&mut self, input: Range<usize>) {
        if input.is_empty() {
            return;
        }
        self.pieces.push(Piece {
            logical_start: self.len,
            input: input.clone(),
        });
        self.len += input.len();
    }

    fn piece_at(&self, offset: usize) -> Option<usize> {
        (offset < self.len).then(|| self.pieces.partition_point(|p| p.logical_start <= offset) - 1)
    }

    pub fn byte(&self, input: &[u8], offset: usize) -> Result<u8> {
        let piece = &self.pieces[self.piece_at(offset).ok_or(Error::Truncated)?];
        Ok(input[piece.input.start + offset - piece.logical_start])
    }

    /// Visit only the pieces covered by this logical range. A NAL or AAC frame
    /// may span many TS packets, so a single contiguous slice is not enough.
    pub fn visit(&self, input: &[u8], range: Range<usize>, mut f: impl FnMut(&[u8])) {
        if range.is_empty() {
            return;
        }
        debug_assert!(range.end <= self.len);
        let first = self.piece_at(range.start).expect("nonempty checked range");
        for piece in &self.pieces[first..] {
            if piece.logical_start >= range.end {
                break;
            }
            let from = range.start.saturating_sub(piece.logical_start);
            let to = (range.end - piece.logical_start).min(piece.input.len());
            f(&input[piece.input.start + from..piece.input.start + to]);
        }
    }

    pub fn copy(&self, input: &[u8], range: Range<usize>, out: &mut Vec<u8>) {
        self.visit(input, range, |bytes| out.extend_from_slice(bytes));
    }

    pub fn small_copy(&self, input: &[u8], range: Range<usize>) -> Vec<u8> {
        let mut out = Vec::with_capacity(range.len());
        self.copy(input, range, &mut out);
        out
    }

    /// Small fixed headers can straddle packets too. Copy them onto the stack
    /// rather than making a heap allocation for each audio frame or PES packet.
    pub fn header<const N: usize>(&self, input: &[u8], at: usize) -> Result<[u8; N]> {
        if at.checked_add(N).is_none_or(|end| end > self.len) {
            return Err(Error::Truncated);
        }
        let mut bytes = [0; N];
        let mut written = 0;
        self.visit(input, at..at + N, |part| {
            bytes[written..written + part.len()].copy_from_slice(part);
            written += part.len();
        });
        Ok(bytes)
    }

    /// Add payload from a completed PES without copying its bytes. Header bytes
    /// and optional padding at the end of that PES are left out of the stream.
    pub fn append_range(&mut self, other: &Self, range: Range<usize>) {
        if range.is_empty() {
            return;
        }
        let first = other.piece_at(range.start).expect("nonempty checked range");
        for piece in &other.pieces[first..] {
            if piece.logical_start >= range.end {
                break;
            }
            let from = range.start.saturating_sub(piece.logical_start);
            let to = (range.end - piece.logical_start).min(piece.input.len());
            self.push(piece.input.start + from..piece.input.start + to);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_copies_across_transport_headers() {
        let input = b"abc__def___ghi";
        let mut stream = Stream::default();
        stream.push(0..3);
        stream.push(5..8);
        stream.push(11..14);
        assert_eq!(stream.small_copy(input, 1..8), b"bcdefgh");
        assert_eq!(stream.byte(input, 3), Ok(b'd'));
        assert_eq!(stream.byte(input, 9), Err(Error::Truncated));
        let mut payload = Stream::default();
        payload.append_range(&stream, 2..7);
        assert_eq!(payload.small_copy(input, 0..5), b"cdefg");
    }
}
