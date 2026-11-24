//! Wire framing for the Buds Core RFCOMM channel.
//!
//! ```text
//! +-----+----------------+----+---------+--------+-----+
//! | SOM | header (u16le) | id | payload | crc16  | EOM |
//! | FD  | size|flags     |    |  ...    | (le)   | DD  |
//! +-----+----------------+----+---------+--------+-----+
//! size  = 1 (id) + payload.len() + 2 (crc)      -> header bits 0..=9
//! 0x1000 = "response" flag, 0x2000 = fragment flag
//! crc   = CRC-16/XMODEM over [id, payload...]
//! ```

pub const SOM: u8 = 0xFD;
pub const EOM: u8 = 0xDD;

const FLAG_RESPONSE: u16 = 0x1000;
const FLAG_FRAGMENT: u16 = 0x2000;
const SIZE_MASK: u16 = 0x03FF;
/// Largest frame the 10 bit size field can describe.
pub const MAX_FRAME_LEN: usize = 1 + 2 + SIZE_MASK as usize + 1;

/// CRC-16/XMODEM (poly 0x1021, init 0x0000, no reflection).
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0;
    for &b in data {
        crc ^= (b as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 { (crc << 1) ^ 0x1021 } else { crc << 1 };
        }
    }
    crc
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub id: u8,
    pub payload: Vec<u8>,
    /// Raw 0x1000 header bit. Messages coming *from* the earbuds have it set.
    pub response_flag: bool,
    pub fragment: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    BadCrc { id: u8, expected: u16, actual: u16 },
    BadEom { id: u8, got: u8 },
    TooShort,
    /// Garbage bytes that were skipped while looking for the next SOM.
    Skipped(usize),
}

impl Frame {
    pub fn request(id: u8, payload: impl Into<Vec<u8>>) -> Self {
        Frame { id, payload: payload.into(), response_flag: false, fragment: false }
    }

    pub fn encode(&self) -> Vec<u8> {
        let size = (1 + self.payload.len() + 2) as u16;
        assert!(size <= SIZE_MASK, "payload too large for a single frame");
        let mut header = size;
        if self.response_flag {
            header |= FLAG_RESPONSE;
        }
        if self.fragment {
            header |= FLAG_FRAGMENT;
        }
        let mut out = Vec::with_capacity(self.payload.len() + 7);
        out.push(SOM);
        out.extend_from_slice(&header.to_le_bytes());
        out.push(self.id);
        out.extend_from_slice(&self.payload);
        let mut crc_input = Vec::with_capacity(self.payload.len() + 1);
        crc_input.push(self.id);
        crc_input.extend_from_slice(&self.payload);
        out.extend_from_slice(&crc16(&crc_input).to_le_bytes());
        out.push(EOM);
        out
    }

    /// Decode exactly one complete frame.
    pub fn decode(raw: &[u8]) -> Result<Frame, FrameError> {
        let mut d = FrameDecoder::default();
        let mut out = d.push(raw);
        match out.iter().position(|r| r.is_ok()) {
            Some(i) => out.swap_remove(i),
            None => out.pop().unwrap_or(Err(FrameError::TooShort)),
        }
    }
}

/// Streaming decoder: RFCOMM gives us arbitrary chunks, frames can be split
/// across reads or several can arrive in one read.
#[derive(Default, Debug)]
pub struct FrameDecoder {
    buf: Vec<u8>,
}

impl FrameDecoder {
    pub fn push(&mut self, data: &[u8]) -> Vec<Result<Frame, FrameError>> {
        self.buf.extend_from_slice(data);
        let mut out = Vec::new();
        loop {
            // Resync on SOM.
            match self.buf.iter().position(|&b| b == SOM) {
                None => {
                    if !self.buf.is_empty() {
                        out.push(Err(FrameError::Skipped(self.buf.len())));
                        self.buf.clear();
                    }
                    break;
                }
                Some(0) => {}
                Some(n) => {
                    out.push(Err(FrameError::Skipped(n)));
                    self.buf.drain(..n);
                }
            }
            if self.buf.len() < 3 {
                break;
            }
            let header = u16::from_le_bytes([self.buf[1], self.buf[2]]);
            let size = (header & SIZE_MASK) as usize;
            if size < 3 {
                // Not a real frame start; drop this SOM and keep looking.
                out.push(Err(FrameError::Skipped(1)));
                self.buf.drain(..1);
                continue;
            }
            let total = 1 + 2 + size + 1;
            if self.buf.len() < total {
                break; // wait for more bytes
            }
            let id = self.buf[3];
            let payload = self.buf[4..3 + size - 2].to_vec();
            let crc_lo = self.buf[3 + size - 2];
            let crc_hi = self.buf[3 + size - 1];
            let actual = u16::from_le_bytes([crc_lo, crc_hi]);
            let eom = self.buf[total - 1];

            let mut crc_input = Vec::with_capacity(payload.len() + 1);
            crc_input.push(id);
            crc_input.extend_from_slice(&payload);
            let expected = crc16(&crc_input);

            if eom != EOM {
                out.push(Err(FrameError::BadEom { id, got: eom }));
                self.buf.drain(..1);
                continue;
            }
            if expected != actual {
                out.push(Err(FrameError::BadCrc { id, expected, actual }));
                self.buf.drain(..1);
                continue;
            }
            out.push(Ok(Frame {
                id,
                payload,
                response_flag: header & FLAG_RESPONSE != 0,
                fragment: header & FLAG_FRAGMENT != 0,
            }));
            self.buf.drain(..total);
        }
        out
    }

    pub fn pending(&self) -> usize {
        self.buf.len()
    }

    pub fn clear(&mut self) {
        self.buf.clear();
    }
}

/// Scan an arbitrary byte blob (e.g. a btsnoop HCI log) for valid frames.
/// Returns (offset, frame) pairs. Useful for reverse engineering captures.
pub fn scan(blob: &[u8]) -> Vec<(usize, Frame)> {
    let mut found = Vec::new();
    let mut i = 0;
    while i + 7 <= blob.len() {
        if blob[i] == SOM {
            let header = u16::from_le_bytes([blob[i + 1], blob[i + 2]]);
            let size = (header & SIZE_MASK) as usize;
            let total = 4 + size;
            if size >= 3 && i + total <= blob.len() && blob[i + total - 1] == EOM {
                if let Ok(f) = Frame::decode(&blob[i..i + total]) {
                    found.push((i, f));
                    i += total;
                    continue;
                }
            }
        }
        i += 1;
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_known_vector() {
        // CRC-16/XMODEM check value
        assert_eq!(crc16(b"123456789"), 0x31C3);
    }

    #[test]
    fn roundtrip() {
        let f = Frame::request(0x78, vec![1]);
        let raw = f.encode();
        assert_eq!(raw[0], SOM);
        assert_eq!(*raw.last().unwrap(), EOM);
        assert_eq!(Frame::decode(&raw).unwrap(), f);
    }

    #[test]
    fn split_and_merged_chunks() {
        let a = Frame::request(0x78, vec![2]).encode();
        let b = Frame::request(0x90, vec![1, 0, 1, 1, 1, 0, 0]).encode();
        let mut stream = vec![0x00, 0x13]; // junk
        stream.extend(&a);
        stream.extend(&b);
        let mut d = FrameDecoder::default();
        let mut frames = vec![];
        for chunk in stream.chunks(3) {
            frames.extend(d.push(chunk).into_iter().filter_map(Result::ok));
        }
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].id, 0x78);
        assert_eq!(frames[1].id, 0x90);
        assert_eq!(d.pending(), 0);
    }

    #[test]
    fn corrupt_crc_resyncs() {
        let mut bad = Frame::request(0x78, vec![2]).encode();
        bad[5] ^= 0x5A; // corrupt the CRC
        let good = Frame::request(0x78, vec![1]).encode();
        let mut d = FrameDecoder::default();
        let mut s = bad.clone();
        s.extend(&good);
        let res = d.push(&s);
        assert!(res.iter().any(|r| matches!(r, Err(FrameError::BadCrc { .. }))));
        let ok: Vec<_> = res.into_iter().filter_map(Result::ok).collect();
        assert_eq!(ok, vec![Frame::request(0x78, vec![1])]);
    }

    #[test]
    fn scan_finds_frames_in_noise() {
        let mut blob = vec![0xAA; 40];
        blob.extend(Frame::request(0xA0, vec![]).encode());
        blob.extend(vec![0x01, 0xFD, 0x02]);
        blob.extend(Frame::request(0x86, vec![3]).encode());
        let hits = scan(&blob);
        assert_eq!(hits.iter().map(|(_, f)| f.id).collect::<Vec<_>>(), vec![0xA0, 0x86]);
    }
}
