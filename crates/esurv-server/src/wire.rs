//! World packets on the socket: building a body, and the two header layouts (the server's is 4
//! bytes, size big endian counting the opcode, then the opcode; the client's is 6, with a 4-byte
//! opcode). After the handshake both headers run through the session key's cipher.

use std::io::{self, Read, Write};
use std::net::TcpStream;

use benilla_srp::{DecrypterHalf, EncrypterHalf};

/// One server packet.
#[derive(Debug, Clone, PartialEq)]
pub struct Packet {
    pub opcode: u16,
    pub body: Vec<u8>,
}

/// A packet body being written, little endian throughout.
#[derive(Debug, Default, Clone)]
pub struct Body(pub Vec<u8>);

impl Body {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.0.push(v);
        self
    }

    pub fn u16(&mut self, v: u16) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn i32(&mut self, v: i32) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn u64(&mut self, v: u64) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn f32(&mut self, v: f32) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn vec3(&mut self, v: [f32; 3]) -> &mut Self {
        self.f32(v[0]).f32(v[1]).f32(v[2])
    }

    /// A NUL-terminated string.
    pub fn cstr(&mut self, s: &str) -> &mut Self {
        self.0.extend_from_slice(s.as_bytes());
        self.0.push(0);
        self
    }

    /// A guid as a mask byte and its non-zero bytes.
    pub fn packed_guid(&mut self, guid: u64) -> &mut Self {
        let mut mask = 0u8;
        let mut bytes = Vec::with_capacity(8);
        for (i, b) in guid.to_le_bytes().into_iter().enumerate() {
            if b != 0 {
                mask |= 1 << i;
                bytes.push(b);
            }
        }
        self.0.push(mask);
        self.0.extend_from_slice(&bytes);
        self
    }

    pub fn bytes(&mut self, b: &[u8]) -> &mut Self {
        self.0.extend_from_slice(b);
        self
    }

    pub fn packet(&mut self, opcode: u16) -> Packet {
        Packet {
            opcode,
            body: std::mem::take(&mut self.0),
        }
    }
}

/// The sending half of a world socket.
pub struct PacketWriter {
    stream: TcpStream,
    cipher: Option<EncrypterHalf>,
}

impl PacketWriter {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            cipher: None,
        }
    }

    /// From now on headers are encrypted.
    pub fn encrypt_with(&mut self, cipher: EncrypterHalf) {
        self.cipher = Some(cipher);
    }

    pub fn send(&mut self, packet: &Packet) -> io::Result<()> {
        let size = u16::try_from(packet.body.len() + 2)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "packet too large"))?;
        let s = size.to_be_bytes();
        let o = packet.opcode.to_le_bytes();
        let mut header = [s[0], s[1], o[0], o[1]];
        if let Some(c) = &mut self.cipher {
            c.encrypt(&mut header);
        }
        let mut out = Vec::with_capacity(4 + packet.body.len());
        out.extend_from_slice(&header);
        out.extend_from_slice(&packet.body);
        self.stream.write_all(&out)
    }

    /// Closes the socket both ways (the reading thread wakes up and ends).
    pub fn close(&self) {
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }
}

/// The receiving half of a world socket.
pub struct PacketReader {
    stream: TcpStream,
    cipher: Option<DecrypterHalf>,
}

impl PacketReader {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            cipher: None,
        }
    }

    pub fn decrypt_with(&mut self, cipher: DecrypterHalf) {
        self.cipher = Some(cipher);
    }

    /// The next client packet's opcode and body.
    pub fn recv(&mut self) -> io::Result<(u16, Vec<u8>)> {
        let mut header = [0u8; 6];
        self.stream.read_exact(&mut header)?;
        if let Some(c) = &mut self.cipher {
            c.decrypt(&mut header);
        }
        let size = u16::from_be_bytes([header[0], header[1]]) as usize;
        let opcode = u32::from_le_bytes([header[2], header[3], header[4], header[5]]);
        let mut body = vec![0u8; size.saturating_sub(4)];
        self.stream.read_exact(&mut body)?;
        Ok((opcode as u16, body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_packed_guid_keeps_only_the_non_zero_bytes() {
        let mut b = Body::new();
        b.packed_guid(0xF130_0000_0000_0042);
        assert_eq!(b.0, [0b1100_0001, 0x42, 0x30, 0xF1]);
        let mut r = b.0.as_slice();
        assert_eq!(
            benilla_protocol::wire::read_packed_guid(&mut r).unwrap(),
            0xF130_0000_0000_0042
        );
    }
}
