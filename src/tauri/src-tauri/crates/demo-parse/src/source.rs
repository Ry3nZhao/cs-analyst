//! Demo file access: plain `.dem` files are memory-mapped, zstd-compressed demos (`.dem.zst`,
//! written by the app's storage policy) are decompressed into memory. Detection is by the zstd
//! frame magic, not the extension, so a renamed file still works.

use memmap2::{Mmap, MmapOptions};
use std::io::Read;
use std::ops::Deref;
use std::path::Path;

/// First four bytes of every zstd frame.
pub const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

/// Bytes of a demo, borrowed from a memory map or owned after decompression.
pub enum DemoBytes {
    Mapped(Mmap),
    Owned(Vec<u8>),
}

impl Deref for DemoBytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            DemoBytes::Mapped(m) => m,
            DemoBytes::Owned(v) => v,
        }
    }
}

/// True when the file starts with a zstd frame.
pub fn is_zstd_file(path: &Path) -> std::io::Result<bool> {
    let mut head = [0u8; 4];
    let mut f = std::fs::File::open(path)?;
    let n = f.read(&mut head)?;
    Ok(n == 4 && head == ZSTD_MAGIC)
}

/// Load a demo for parsing: mmap a raw `.dem`, or stream-decompress a `.dem.zst` into memory.
pub fn read_demo(path: &Path) -> std::io::Result<DemoBytes> {
    if is_zstd_file(path)? {
        let file = std::fs::File::open(path)?;
        let size_hint = file.metadata().map(|m| m.len() as usize * 3).unwrap_or(0);
        let mut out = Vec::with_capacity(size_hint);
        zstd::stream::read::Decoder::new(file)?.read_to_end(&mut out)?;
        return Ok(DemoBytes::Owned(out));
    }
    let file = std::fs::File::open(path)?;
    // SAFETY: the file is opened read-only and not modified while mapped.
    let mmap = unsafe { MmapOptions::new().map(&file)? };
    Ok(DemoBytes::Mapped(mmap))
}

/// Explicit entry for compressed demos (same as [`crate::parse_demo_file`], which auto-detects).
pub fn parse_demo_zst(path: &Path) -> Result<crate::ParsedDemo, crate::ParseError> {
    let file = std::fs::File::open(path)?;
    let mut out = Vec::new();
    zstd::stream::read::Decoder::new(file)?.read_to_end(&mut out)?;
    crate::parse_demo_bytes(&out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_and_decompresses_zstd() {
        let dir = std::env::temp_dir().join(format!("demo-parse-src-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let raw = dir.join("a.dem");
        let zst = dir.join("a.dem.zst");
        let payload: Vec<u8> = (0..100_000u32).flat_map(|i| (i % 251).to_le_bytes()).collect();
        std::fs::write(&raw, &payload).unwrap();
        std::fs::write(&zst, zstd::encode_all(&payload[..], 3).unwrap()).unwrap();
        assert!(!is_zstd_file(&raw).unwrap());
        assert!(is_zstd_file(&zst).unwrap());
        assert_eq!(&*read_demo(&raw).unwrap(), &payload[..]);
        assert_eq!(&*read_demo(&zst).unwrap(), &payload[..]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
