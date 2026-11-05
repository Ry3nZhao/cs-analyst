//! Binary payload of `get_replay`, returned as a raw `ArrayBuffer` through Tauri's binary IPC
//! (`tauri::ipc::Response`), so nothing is JSON- or base64-encoded and the frontend views every
//! column zero-copy as a typed array.
//!
//! ```text
//! 0   [u8; 4]  magic b"CSRP"
//! 4   u32      PACK_VERSION
//! 8   u32      header length H (JSON, UTF-8, space-padded to a multiple of 4)
//! 12  [u8; H]  header JSON (`ReplayHeader`, camelCase) — players, rounds, kills, grenades, bomb
//! 12+H         body; `header.columns.<name>` = byte offset from the body start (4-byte aligned)
//!     ticks      i32 × nFrames
//!     x, y, z    f32 × nFrames·nPlayers   (frame-major: frame * nPlayers + slot)
//!     yaw        f32 × nFrames·nPlayers   (degrees)
//!     health     u8  × nFrames·nPlayers
//!     armor      u8  × nFrames·nPlayers
//!     flags      u8  × nFrames·nPlayers   bit0 alive, bit1 CT, bit2 scoped, bit3 flashed
//!     spottedBy  u16 × nFrames·nPlayers   bit i = slot i sees this player
//! ```
//! Why one binary blob instead of per-round JSON: a 30-round match is ~35k frames × 10 players
//! ≈ 7 MB as typed arrays but 30+ MB as JSON (and slow to parse); base64 adds 33 % and a decode
//! pass. Scrubbing the timeline across rounds needs everything anyway, and the in-process IPC
//! copy of a few MB takes milliseconds. The file on disk stays zstd-compressed (1–2 MB).

use demo_parse::Replay;
use serde::Serialize;

pub const MAGIC: &[u8; 4] = b"CSRP";
pub const PACK_VERSION: u32 = 1;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Columns {
    pub ticks: usize,
    pub x: usize,
    pub y: usize,
    pub z: usize,
    pub yaw: usize,
    pub health: usize,
    pub armor: usize,
    pub flags: usize,
    pub spotted_by: usize,
    pub body_len: usize,
}

fn pad4(b: &mut Vec<u8>) {
    while b.len() % 4 != 0 {
        b.push(0);
    }
}

/// Body bytes and the column offsets.
pub fn body(r: &Replay) -> (Vec<u8>, Columns) {
    let n = r.n_frames() * r.n_players();
    let mut b = Vec::with_capacity(r.n_frames() * 4 + n * 21 + 16);
    let mut c = Columns { ticks: 0, ..Default::default() };
    for t in &r.ticks {
        b.extend_from_slice(&t.to_le_bytes());
    }
    let f32col = |b: &mut Vec<u8>, v: &[f32]| -> usize {
        let at = b.len();
        for x in v {
            b.extend_from_slice(&x.to_le_bytes());
        }
        at
    };
    c.x = f32col(&mut b, &r.x);
    c.y = f32col(&mut b, &r.y);
    c.z = f32col(&mut b, &r.z);
    c.yaw = f32col(&mut b, &r.yaw);
    for (slot, v) in [(&mut c.health, &r.health), (&mut c.armor, &r.armor), (&mut c.flags, &r.flags)] {
        *slot = b.len();
        b.extend_from_slice(v);
        pad4(&mut b);
    }
    c.spotted_by = b.len();
    for v in &r.spotted_by {
        b.extend_from_slice(&v.to_le_bytes());
    }
    pad4(&mut b);
    c.body_len = b.len();
    (b, c)
}

/// Full payload: magic, version, padded header JSON, body.
pub fn pack(header_json: &[u8], body: &[u8]) -> Vec<u8> {
    let mut h = header_json.to_vec();
    while h.len() % 4 != 0 {
        h.push(b' ');
    }
    let mut out = Vec::with_capacity(12 + h.len() + body.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&PACK_VERSION.to_le_bytes());
    out.extend_from_slice(&(h.len() as u32).to_le_bytes());
    out.extend_from_slice(&h);
    out.extend_from_slice(body);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_is_aligned() {
        let r = Replay {
            tick_step: 4,
            steamids: vec![1, 2, 3],
            ticks: vec![10, 14],
            x: vec![1.5; 6],
            y: vec![2.0; 6],
            z: vec![0.0; 6],
            yaw: vec![90.0; 6],
            health: vec![100; 6],
            armor: vec![50; 6],
            flags: vec![1; 6],
            spotted_by: vec![7; 6],
        };
        let (b, c) = body(&r);
        for off in [c.ticks, c.x, c.y, c.z, c.yaw, c.health, c.armor, c.flags, c.spotted_by, c.body_len] {
            assert_eq!(off % 4, 0);
        }
        assert_eq!(c.x, 8);
        assert_eq!(f32::from_le_bytes(b[c.x..c.x + 4].try_into().unwrap()), 1.5);
        assert_eq!(b[c.armor], 50);
        assert_eq!(u16::from_le_bytes([b[c.spotted_by + 10], b[c.spotted_by + 11]]), 7);
        assert_eq!(c.body_len, b.len());
        let p = pack(b"{\"a\":1}", &b);
        assert_eq!(&p[..4], MAGIC);
        let hl = u32::from_le_bytes(p[8..12].try_into().unwrap()) as usize;
        assert_eq!(hl, 8);
        assert_eq!(p.len(), 12 + hl + b.len());
    }
}
