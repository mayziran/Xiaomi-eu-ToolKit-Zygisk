use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

pub const MAGIC: [u8; 4] = *b"MPZK";
pub const VERSION: u16 = 1;
pub const MAX_FRAME: u32 = 4096;
pub const QUERY: u16 = 1;
pub const RESPONSE: u16 = 2;
pub const STATUS_OK: u8 = 0;
pub const STATUS_REJECTED: u8 = 1;

pub fn configure(stream: &UnixStream) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    Ok(())
}

pub fn write_frame(stream: &mut UnixStream, kind: u16, payload: &[u8]) -> io::Result<()> {
    if payload.len() > MAX_FRAME as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "frame too large",
        ));
    }
    let mut header = [0u8; 16];
    header[0..4].copy_from_slice(&MAGIC);
    header[4..6].copy_from_slice(&VERSION.to_be_bytes());
    header[6..8].copy_from_slice(&kind.to_be_bytes());
    header[8..12].copy_from_slice(&(payload.len() as u32).to_be_bytes());
    stream.write_all(&header)?;
    stream.write_all(payload)?;
    stream.flush()
}

pub fn read_frame(stream: &mut UnixStream) -> io::Result<(u16, Vec<u8>)> {
    let mut header = [0u8; 16];
    stream.read_exact(&mut header)?;
    if header[0..4] != MAGIC {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "bad magic"));
    }
    if u16::from_be_bytes([header[4], header[5]]) != VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported version",
        ));
    }
    let length = u32::from_be_bytes([header[8], header[9], header[10], header[11]]);
    if length > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame too large",
        ));
    }
    let mut payload = vec![0u8; length as usize];
    stream.read_exact(&mut payload)?;
    Ok((u16::from_be_bytes([header[6], header[7]]), payload))
}

pub fn encode_query(package: &str, process: &str) -> Vec<u8> {
    format!("{package}\n{process}").into_bytes()
}

pub fn decode_query(payload: &[u8]) -> io::Result<(&str, &str)> {
    let text = std::str::from_utf8(payload)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "query is not utf8"))?;
    let mut lines = text.splitn(2, '\n');
    let package = lines.next().unwrap_or("").trim();
    let process = lines.next().unwrap_or("").trim();
    if package.is_empty() || process.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing identity",
        ));
    }
    Ok((package, process))
}

pub fn encode_response(should_hook: bool) -> Vec<u8> {
    vec![if should_hook {
        STATUS_OK
    } else {
        STATUS_REJECTED
    }]
}

pub fn decode_response(payload: &[u8]) -> io::Result<bool> {
    match payload {
        [STATUS_OK] => Ok(true),
        [STATUS_REJECTED] => Ok(false),
        _ => Err(io::Error::new(io::ErrorKind::InvalidData, "bad response")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_round_trip() {
        let encoded = encode_query("com.example.app", "com.example.app:push");
        assert_eq!(
            decode_query(&encoded).unwrap(),
            ("com.example.app", "com.example.app:push")
        );
    }

    #[test]
    fn rejects_oversized_payload() {
        let (left, _right) = UnixStream::pair().unwrap();
        let mut left = left;
        assert!(write_frame(&mut left, QUERY, &vec![0; MAX_FRAME as usize + 1]).is_err());
    }
}
