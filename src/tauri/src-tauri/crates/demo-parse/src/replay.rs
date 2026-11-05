//! Sampled per-player frames for 2D replay / heatmaps.
//!
//! File layout (whole payload zstd-compressed), all little-endian:
//! ```text
//! magic  b"CSAREPL1"
//! u16    format version (1)
//! u16    tick_step
//! u32    n_frames
//! u8     n_players
//! u64    steamid × n_players               (slot order)
//! i32    tick × n_frames
//! then per field, n_frames × n_players values in frame-major order (frame * n_players + slot):
//!   f32 x, f32 y, f32 z, f32 yaw, u8 health, u8 armor, u8 flags, u16 spotted_by
//! ```
//! `flags`: bit0 alive, bit1 CT (else T), bit2 scoped, bit3 flashed.
//! `spotted_by`: bit i set = player in slot i can see this player.

use serde::Serialize;
use std::io::{Read, Write};
use std::path::Path;

const MAGIC: &[u8; 8] = b"CSAREPL1";
const VERSION: u16 = 1;

pub const FLAG_ALIVE: u8 = 1;
pub const FLAG_CT: u8 = 1 << 1;
pub const FLAG_SCOPED: u8 = 1 << 2;
pub const FLAG_FLASHED: u8 = 1 << 3;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Replay {
    pub tick_step: u16,
    pub steamids: Vec<u64>,
    pub ticks: Vec<i32>,
    pub x: Vec<f32>,
    pub y: Vec<f32>,
    pub z: Vec<f32>,
    pub yaw: Vec<f32>,
    pub health: Vec<u8>,
    pub armor: Vec<u8>,
    pub flags: Vec<u8>,
    pub spotted_by: Vec<u16>,
}

#[derive(Debug, thiserror::Error)]
pub enum ReplayError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid replay file: {0}")]
    Invalid(&'static str),
}

impl Replay {
    pub fn n_players(&self) -> usize {
        self.steamids.len()
    }

    pub fn n_frames(&self) -> usize {
        self.ticks.len()
    }

    pub fn idx(&self, frame: usize, slot: usize) -> usize {
        frame * self.n_players() + slot
    }

    fn encode(&self) -> Vec<u8> {
        let np = self.n_players();
        let nf = self.n_frames();
        let mut b = Vec::with_capacity(32 + np * 8 + nf * 4 + nf * np * 21);
        b.extend_from_slice(MAGIC);
        b.extend_from_slice(&VERSION.to_le_bytes());
        b.extend_from_slice(&self.tick_step.to_le_bytes());
        b.extend_from_slice(&(nf as u32).to_le_bytes());
        b.push(np as u8);
        for s in &self.steamids {
            b.extend_from_slice(&s.to_le_bytes());
        }
        for t in &self.ticks {
            b.extend_from_slice(&t.to_le_bytes());
        }
        for col in [&self.x, &self.y, &self.z, &self.yaw] {
            for v in col.iter() {
                b.extend_from_slice(&v.to_le_bytes());
            }
        }
        b.extend_from_slice(&self.health);
        b.extend_from_slice(&self.armor);
        b.extend_from_slice(&self.flags);
        for v in &self.spotted_by {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b
    }

    fn decode(raw: &[u8]) -> Result<Self, ReplayError> {
        let mut r = Cursor { buf: raw, pos: 0 };
        if r.take(8)? != MAGIC {
            return Err(ReplayError::Invalid("magic"));
        }
        if r.u16()? != VERSION {
            return Err(ReplayError::Invalid("version"));
        }
        let tick_step = r.u16()?;
        let nf = r.u32()? as usize;
        let np = r.take(1)?[0] as usize;
        let steamids = (0..np).map(|_| r.u64()).collect::<Result<_, _>>()?;
        let ticks = (0..nf).map(|_| r.i32()).collect::<Result<_, _>>()?;
        let n = nf * np;
        let mut f32s = || (0..n).map(|_| r.f32()).collect::<Result<Vec<_>, _>>();
        let x = f32s()?;
        let y = f32s()?;
        let z = f32s()?;
        let yaw = f32s()?;
        let health = r.take(n)?.to_vec();
        let armor = r.take(n)?.to_vec();
        let flags = r.take(n)?.to_vec();
        let spotted_by = (0..n).map(|_| r.u16()).collect::<Result<_, _>>()?;
        Ok(Self { tick_step, steamids, ticks, x, y, z, yaw, health, armor, flags, spotted_by })
    }

    pub fn write_to(&self, path: &Path) -> Result<u64, ReplayError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("tmp");
        {
            let file = std::fs::File::create(&tmp)?;
            let mut enc = zstd::Encoder::new(file, 6)?;
            enc.write_all(&self.encode())?;
            enc.finish()?.sync_all()?;
        }
        std::fs::rename(&tmp, path)?;
        Ok(std::fs::metadata(path)?.len())
    }

    pub fn read_from(path: &Path) -> Result<Self, ReplayError> {
        let mut raw = Vec::new();
        zstd::Decoder::new(std::fs::File::open(path)?)?.read_to_end(&mut raw)?;
        Self::decode(&raw)
    }
}

struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], ReplayError> {
        let end = self.pos.checked_add(n).filter(|e| *e <= self.buf.len()).ok_or(ReplayError::Invalid("truncated"))?;
        let s = &self.buf[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn u16(&mut self) -> Result<u16, ReplayError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, ReplayError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32, ReplayError> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, ReplayError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn f32(&mut self) -> Result<f32, ReplayError> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let r = Replay {
            tick_step: 4,
            steamids: vec![1, 2],
            ticks: vec![10, 14],
            x: vec![1.0, 2.0, 3.0, 4.0],
            y: vec![0.5; 4],
            z: vec![-1.0; 4],
            yaw: vec![90.0; 4],
            health: vec![100, 50, 0, 1],
            armor: vec![100; 4],
            flags: vec![FLAG_ALIVE, FLAG_ALIVE | FLAG_CT, 0, FLAG_CT],
            spotted_by: vec![0, 1, 2, 3],
        };
        let dir = std::env::temp_dir().join(format!("demo-parse-replay-{}", std::process::id()));
        let path = dir.join("r.replay.zst");
        r.write_to(&path).unwrap();
        assert_eq!(Replay::read_from(&path).unwrap(), r);
        let _ = std::fs::remove_dir_all(dir);
    }
}
