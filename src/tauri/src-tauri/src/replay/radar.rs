//! Radar backdrop for the 2D replay / heatmap: per-map world → image calibration and the image
//! itself.
//!
//! **No Valve image ships with the app.** Images come from (in order):
//! 1. a user folder set on the replay page (`<map>_radar.png|jpg|webp` or `<map>.png|…`,
//!    optional `<map>_lower_radar.*` / `<map>_lower.*` and `<map>.txt` overview file);
//! 2. the user's local CS2 install: `game/csgo/pak01_dir.vpk` holds
//!    `panorama/images/overheadmaps/<map>_radar_psd.vtex_c` (Source 2 texture, 1024², LZ4 +
//!    BGRA8888 or DXT) and `resource/overviews/<map>.txt` (calibration). We decode them once
//!    into `<data_root>/cache/radar/` (the user's own copy, never committed or redistributed).
//!
//! Without an image the frontend draws points on a blank canvas fitted to the data.
//!
//! Calibration (`pos_x` / `pos_y` = world coordinate of the image's top-left corner, `scale` =
//! world units per pixel of the 1024 px image; `lower_max_z` = altitude below which the
//! `_lower` image applies): user `<map>.txt` > CS2 `resource/overviews/<map>.txt` > the built-in
//! [`BUILTIN`] table, which was copied from those same CS2 overview files (game build of
//! 2026-10-31). Pixel = ((x − pos_x) / scale, (pos_y − y) / scale).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// (map, pos_x, pos_y, scale, lower_max_z). Source: CS2 `game/csgo/pak01_dir.vpk` →
/// `resource/overviews/<map>.txt` (`pos_x`, `pos_y`, `scale`,
/// `verticalsections.lower.AltitudeMax`). Add a row to support another map without an install.
pub const BUILTIN: &[(&str, f32, f32, f32, Option<f32>)] = &[
    ("de_ancient", -2953.0, 2164.0, 5.0, None),
    ("de_anubis", -2796.0, 3328.0, 5.22, None),
    ("de_dust2", -2476.0, 3239.0, 4.4, None),
    ("de_inferno", -2087.0, 3870.0, 4.9, None),
    ("de_mirage", -3230.0, 1713.0, 5.0, None),
    ("de_nuke", -3453.0, 2887.0, 7.0, Some(-495.0)),
    ("de_overpass", -4831.0, 1781.0, 5.2, None),
    ("de_train", -2308.0, 2078.0, 4.082077, Some(-50.0)),
    ("de_vertigo", -3168.0, 1762.0, 4.0, Some(11700.0)),
    ("de_cache", -2000.0, 3250.0, 5.5, None),
    ("de_dust", -2850.0, 4073.0, 6.0, None),
    ("cs_office", -1838.0, 1858.0, 4.1, None),
    ("cs_italy", -2647.0, 2592.0, 4.6, None),
    ("ar_baggage", -1316.0, 1288.0, 2.539062, Some(-5.0)),
    ("ar_shoots", -1368.0, 1952.0, 2.6875, None),
];

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct RadarSettings {
    /// Folder with user-supplied radar images (and optional overview `.txt`).
    pub image_dir: Option<String>,
    /// CS2 install folder (`…/Counter-Strike Global Offensive`, or its `game/csgo`). `None` =
    /// detect through Steam.
    pub cs2_dir: Option<String>,
    /// Set to `false` to never read the CS2 install.
    pub use_cs2: Option<bool>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Calibration {
    pub pos_x: f32,
    pub pos_y: f32,
    pub scale: f32,
    pub lower_max_z: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RadarLevel {
    /// `default` | `lower`
    pub level: String,
    /// `user` | `cs2`
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RadarInfo {
    pub map_name: String,
    pub calibration: Option<Calibration>,
    /// `user` | `cs2` | `builtin`
    pub calibration_source: Option<String>,
    /// Available images; empty = draw without a backdrop.
    pub levels: Vec<RadarLevel>,
    /// CS2 `game/csgo` folder in use, if found.
    pub cs2_dir: Option<String>,
    pub settings: RadarSettings,
    /// Why no image is available / what went wrong reading the install (for the UI).
    pub note: Option<String>,
}

/// Map names become file names: keep `[a-z0-9_]` only.
pub fn sanitize_map(map: &str) -> Option<String> {
    let m = map.trim().to_ascii_lowercase();
    (!m.is_empty() && m.len() <= 64 && m.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')).then_some(m)
}

pub fn builtin(map: &str) -> Option<Calibration> {
    BUILTIN.iter().find(|r| r.0 == map).map(|r| Calibration { pos_x: r.1, pos_y: r.2, scale: r.3, lower_max_z: r.4 })
}

// ---------------------------------------------------------------------------------------------
// Settings file: <data_root>/state/radar.json (no DB table needed)
// ---------------------------------------------------------------------------------------------

fn settings_path(data_root: &Path) -> PathBuf {
    data_root.join("state").join("radar.json")
}

pub fn load_settings(data_root: &Path) -> RadarSettings {
    fs::read(settings_path(data_root)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn save_settings(data_root: &Path, s: &RadarSettings) -> std::io::Result<()> {
    let p = settings_path(data_root);
    if let Some(d) = p.parent() {
        fs::create_dir_all(d)?;
    }
    fs::write(p, serde_json::to_vec_pretty(s).unwrap_or_default())
}

// ---------------------------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------------------------

const IMG_EXT: &[&str] = &["png", "jpg", "jpeg", "webp"];

fn find_user_image(dir: &Path, map: &str, level: &str) -> Option<PathBuf> {
    let stems: Vec<String> = if level == "lower" {
        vec![format!("{map}_lower_radar"), format!("{map}_lower")]
    } else {
        vec![format!("{map}_radar"), map.to_string()]
    };
    for s in &stems {
        for e in IMG_EXT {
            let p = dir.join(format!("{s}.{e}"));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

fn cache_dir(data_root: &Path) -> PathBuf {
    data_root.join("cache").join("radar")
}

/// Everything about a map's radar. Extracts from the CS2 install into the cache when needed.
pub fn resolve(data_root: &Path, map: &str) -> RadarInfo {
    let settings = load_settings(data_root);
    let mut info = RadarInfo {
        map_name: map.to_string(),
        calibration: None,
        calibration_source: None,
        levels: vec![],
        cs2_dir: None,
        settings: settings.clone(),
        note: None,
    };
    let mut user_cal = None;
    if let Some(dir) = settings.image_dir.as_deref().map(PathBuf::from).filter(|d| d.is_dir()) {
        user_cal = fs::read_to_string(dir.join(format!("{map}.txt"))).ok().and_then(|t| parse_overview(&t));
        for level in ["default", "lower"] {
            if find_user_image(&dir, map, level).is_some() {
                info.levels.push(RadarLevel { level: level.into(), source: "user".into() });
            }
        }
    }

    let mut cs2_cal = None;
    if settings.use_cs2 != Some(false) {
        match locate_cs2(settings.cs2_dir.as_deref()) {
            Some(csgo) => {
                info.cs2_dir = Some(csgo.to_string_lossy().into());
                match ensure_cs2_cache(data_root, &csgo, map) {
                    Ok(c) => {
                        cs2_cal = c.calibration;
                        for level in c.levels {
                            if !info.levels.iter().any(|l| l.level == level) {
                                info.levels.push(RadarLevel { level, source: "cs2".into() });
                            }
                        }
                    }
                    Err(e) => info.note = Some(format!("读取 CS2 雷达图失败：{e}")),
                }
            }
            None => info.note = Some("未找到 CS2 安装目录".into()),
        }
    }

    (info.calibration, info.calibration_source) = match (user_cal, cs2_cal, builtin(map)) {
        (Some(c), _, _) => (Some(c), Some("user".into())),
        (None, Some(c), _) => (Some(c), Some("cs2".into())),
        (None, None, Some(c)) => (Some(c), Some("builtin".into())),
        _ => (None, None),
    };
    if info.levels.iter().all(|l| l.level != "default") && info.note.is_none() {
        info.note = Some(format!("没有找到 {map} 的雷达图"));
    }
    if info.calibration.is_none() {
        info.levels.clear();
        info.note = Some(format!("没有 {map} 的坐标换算参数"));
    }
    info
}

/// Bytes of the image for `level`, from the user folder or the CS2 cache.
pub fn image_bytes(data_root: &Path, map: &str, level: &str) -> Option<Vec<u8>> {
    let settings = load_settings(data_root);
    if let Some(dir) = settings.image_dir.as_deref().map(PathBuf::from) {
        if let Some(p) = find_user_image(&dir, map, level) {
            return fs::read(p).ok();
        }
    }
    let name = if level == "lower" { format!("{map}_lower.png") } else { format!("{map}.png") };
    fs::read(cache_dir(data_root).join(name)).ok()
}

// ---------------------------------------------------------------------------------------------
// CS2 install
// ---------------------------------------------------------------------------------------------

const CS2_REL: &str = "steamapps/common/Counter-Strike Global Offensive/game/csgo";

/// The `game/csgo` folder containing `pak01_dir.vpk`.
pub fn locate_cs2(custom: Option<&str>) -> Option<PathBuf> {
    let ok = |p: PathBuf| p.join("pak01_dir.vpk").is_file().then_some(p);
    if let Some(c) = custom.map(str::trim).filter(|c| !c.is_empty()) {
        let c = PathBuf::from(c);
        return ok(c.clone()).or_else(|| ok(c.join("game").join("csgo")));
    }
    steam_libraries().into_iter().find_map(|lib| ok(lib.join(CS2_REL)))
}

fn steam_libraries() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = vec![];
    if let Some(p) = steam_path_from_registry() {
        roots.push(p);
    }
    for d in ["C:/Program Files (x86)/Steam", "C:/Program Files/Steam"] {
        roots.push(PathBuf::from(d));
    }
    let mut libs = vec![];
    for root in roots {
        if let Ok(t) = fs::read_to_string(root.join("steamapps").join("libraryfolders.vdf")) {
            for line in t.lines() {
                let parts: Vec<&str> = line.split('"').collect();
                if parts.len() >= 4 && parts[1].eq_ignore_ascii_case("path") {
                    libs.push(PathBuf::from(parts[3].replace("\\\\", "\\")));
                }
            }
        }
        libs.push(root);
    }
    libs.dedup();
    libs
}

#[cfg(windows)]
fn steam_path_from_registry() -> Option<PathBuf> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new("reg")
        .args(["query", r"HKCU\Software\Valve\Steam", "/v", "SteamPath"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().find(|l| l.contains("SteamPath"))?;
    let v = line.split("REG_SZ").nth(1)?.trim();
    (!v.is_empty()).then(|| PathBuf::from(v))
}

#[cfg(not(windows))]
fn steam_path_from_registry() -> Option<PathBuf> {
    None
}

struct Cs2Cache {
    calibration: Option<Calibration>,
    levels: Vec<String>,
}

/// Extract `<map>.txt` / `<map>.png` / `<map>_lower.png` from the install into the cache, keyed
/// by the size and mtime of `pak01_dir.vpk` (a game update re-extracts).
fn ensure_cs2_cache(data_root: &Path, csgo: &Path, map: &str) -> Result<Cs2Cache, String> {
    let dir = cache_dir(data_root);
    let vpk = csgo.join("pak01_dir.vpk");
    let meta = fs::metadata(&vpk).map_err(|e| e.to_string())?;
    let mtime = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
    let stamp = format!("{}:{}:{}", vpk.display(), meta.len(), mtime);
    let stamp_path = dir.join(format!("{map}.stamp"));
    let txt_path = dir.join(format!("{map}.txt"));
    let read_cached = || {
        let calibration = fs::read_to_string(&txt_path).ok().and_then(|t| parse_overview(&t));
        let levels = [("default", format!("{map}.png")), ("lower", format!("{map}_lower.png"))]
            .into_iter()
            .filter(|(_, f)| dir.join(f).is_file())
            .map(|(l, _)| l.to_string())
            .collect();
        Cs2Cache { calibration, levels }
    };
    if fs::read_to_string(&stamp_path).ok().as_deref() == Some(stamp.as_str()) {
        return Ok(read_cached());
    }

    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    for f in [format!("{map}.txt"), format!("{map}.png"), format!("{map}_lower.png")] {
        let _ = fs::remove_file(dir.join(f));
    }
    let want_txt = format!("resource/overviews/{map}.txt");
    let want_img = format!("panorama/images/overheadmaps/{map}_radar_psd.vtex_c");
    let want_img_tga = format!("panorama/images/overheadmaps/{map}_radar_tga.vtex_c");
    let want_low = format!("panorama/images/overheadmaps/{map}_lower_radar_psd.vtex_c");
    let want = [want_txt.as_str(), want_img.as_str(), want_img_tga.as_str(), want_low.as_str()];
    let pak = Vpk::open(&vpk).map_err(|e| format!("VPK: {e}"))?;
    let entries = pak.find(&want);
    if let Some(e) = entries.get(&want_txt) {
        let b = pak.read(e).map_err(|e| e.to_string())?;
        fs::write(&txt_path, b).map_err(|e| e.to_string())?;
    }
    let mut errors = vec![];
    for (key, out) in [(&want_img, format!("{map}.png")), (&want_img_tga, format!("{map}.png")), (&want_low, format!("{map}_lower.png"))] {
        let Some(e) = entries.get(key) else { continue };
        if dir.join(&out).is_file() {
            continue;
        }
        let res = pak
            .read(e)
            .map_err(|e| e.to_string())
            .and_then(|b| decode_vtex(&b))
            .and_then(|(w, h, rgba)| encode_png(w, h, &rgba))
            .and_then(|png| fs::write(dir.join(&out), png).map_err(|e| e.to_string()));
        if let Err(err) = res {
            errors.push(format!("{key}: {err}"));
        }
    }
    if !errors.is_empty() {
        return Err(errors.join("; "));
    }
    fs::write(&stamp_path, &stamp).map_err(|e| e.to_string())?;
    Ok(read_cached())
}

// ---------------------------------------------------------------------------------------------
// Overview .txt (Valve KeyValues)
// ---------------------------------------------------------------------------------------------

fn kv_tokens(text: &str) -> Vec<String> {
    let mut out = vec![];
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '/' if it.peek() == Some(&'/') => {
                for c in it.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '"' => {
                let mut s = String::new();
                for c in it.by_ref() {
                    if c == '"' {
                        break;
                    }
                    s.push(c);
                }
                out.push(s);
            }
            '{' | '}' => out.push(c.to_string()),
            c if c.is_whitespace() => {}
            c => {
                let mut s = c.to_string();
                while let Some(&n) = it.peek() {
                    if n.is_whitespace() || n == '"' || n == '{' || n == '}' {
                        break;
                    }
                    s.push(n);
                    it.next();
                }
                out.push(s);
            }
        }
    }
    out
}

/// Flatten KeyValues into lowercase `a.b.c` → value (quoted `{` / `}` strings are rare enough
/// in overview files to ignore).
fn kv_flatten(text: &str) -> HashMap<String, String> {
    let toks = kv_tokens(text);
    let mut out = HashMap::new();
    let mut path: Vec<String> = vec![];
    let mut i = 0;
    while i < toks.len() {
        let t = &toks[i];
        if t == "}" {
            path.pop();
            i += 1;
            continue;
        }
        match toks.get(i + 1).map(String::as_str) {
            Some("{") => {
                path.push(t.to_ascii_lowercase());
                i += 2;
            }
            Some(v) if v != "}" => {
                let mut k = path.clone();
                k.push(t.to_ascii_lowercase());
                out.insert(k.join("."), v.to_string());
                i += 2;
            }
            _ => i += 1,
        }
    }
    out
}

/// `pos_x`, `pos_y`, `scale` and the lower section's `AltitudeMax` from an overview file.
pub fn parse_overview(text: &str) -> Option<Calibration> {
    let kv = kv_flatten(text);
    // Keys are `<map>.pos_x`; take the first top-level block that has them.
    let get = |suffix: &str| -> Option<f32> {
        let mut keys: Vec<&String> = kv.keys().filter(|k| k.ends_with(suffix) && k.matches('.').count() == 1).collect();
        keys.sort();
        keys.first().and_then(|k| kv[*k].trim().parse().ok())
    };
    let lower = kv
        .iter()
        .find(|(k, _)| k.ends_with(".verticalsections.lower.altitudemax"))
        .and_then(|(_, v)| v.trim().parse().ok());
    let scale = get(".scale")?;
    (scale > 0.0).then_some(Calibration { pos_x: get(".pos_x")?, pos_y: get(".pos_y")?, scale, lower_max_z: lower })
}

// ---------------------------------------------------------------------------------------------
// VPK v1/v2 directory
// ---------------------------------------------------------------------------------------------

pub struct VpkEntry {
    archive: u16,
    offset: u32,
    len: u32,
    preload: Vec<u8>,
}

pub struct Vpk {
    dir_path: PathBuf,
    tree: Vec<u8>,
    /// Absolute offset of the embedded data section (archive 0x7fff) in the dir file.
    data_start: u64,
}

impl Vpk {
    pub fn open(dir_path: &Path) -> Result<Self, String> {
        let mut f = fs::File::open(dir_path).map_err(|e| e.to_string())?;
        let mut h = [0u8; 12];
        f.read_exact(&mut h).map_err(|e| e.to_string())?;
        let sig = u32::from_le_bytes(h[0..4].try_into().unwrap());
        let ver = u32::from_le_bytes(h[4..8].try_into().unwrap());
        let tree_size = u32::from_le_bytes(h[8..12].try_into().unwrap()) as usize;
        if sig != 0x55aa_1234 || !(ver == 1 || ver == 2) {
            return Err("not a VPK directory".into());
        }
        let header = if ver == 1 { 12 } else { 28 };
        f.seek(SeekFrom::Start(header)).map_err(|e| e.to_string())?;
        let mut tree = vec![0u8; tree_size];
        f.read_exact(&mut tree).map_err(|e| e.to_string())?;
        Ok(Self { dir_path: dir_path.to_path_buf(), tree, data_start: header + tree_size as u64 })
    }

    /// Entries for the requested full paths (`dir/name.ext`).
    pub fn find(&self, wanted: &[&str]) -> HashMap<String, VpkEntry> {
        let mut out = HashMap::new();
        let b = &self.tree;
        let mut pos = 0usize;
        let cstr = |pos: &mut usize| -> Option<String> {
            let end = b[*pos..].iter().position(|&c| c == 0)? + *pos;
            let s = String::from_utf8_lossy(&b[*pos..end]).into_owned();
            *pos = end + 1;
            Some(s)
        };
        'ext: while let Some(ext) = cstr(&mut pos) {
            if ext.is_empty() {
                break;
            }
            while let Some(dir) = cstr(&mut pos) {
                if dir.is_empty() {
                    continue 'ext;
                }
                while let Some(name) = cstr(&mut pos) {
                    if name.is_empty() {
                        break;
                    }
                    if pos + 18 > b.len() {
                        return out;
                    }
                    let preload_len = u16::from_le_bytes([b[pos + 4], b[pos + 5]]) as usize;
                    let archive = u16::from_le_bytes([b[pos + 6], b[pos + 7]]);
                    let offset = u32::from_le_bytes(b[pos + 8..pos + 12].try_into().unwrap());
                    let len = u32::from_le_bytes(b[pos + 12..pos + 16].try_into().unwrap());
                    pos += 18;
                    let full = if dir.trim().is_empty() { format!("{name}.{ext}") } else { format!("{dir}/{name}.{ext}") };
                    if wanted.contains(&full.as_str()) {
                        let preload = b.get(pos..pos + preload_len).unwrap_or_default().to_vec();
                        out.insert(full, VpkEntry { archive, offset, len, preload });
                    }
                    pos += preload_len;
                }
            }
            break;
        }
        out
    }

    pub fn read(&self, e: &VpkEntry) -> std::io::Result<Vec<u8>> {
        let mut out = e.preload.clone();
        if e.len == 0 {
            return Ok(out);
        }
        let (path, start) = if e.archive == 0x7fff {
            (self.dir_path.clone(), self.data_start + e.offset as u64)
        } else {
            let stem = self.dir_path.file_name().and_then(|s| s.to_str()).unwrap_or("pak01_dir.vpk").replace("_dir.vpk", "");
            (self.dir_path.with_file_name(format!("{stem}_{:03}.vpk", e.archive)), e.offset as u64)
        };
        let mut f = fs::File::open(path)?;
        f.seek(SeekFrom::Start(start))?;
        let mut buf = vec![0u8; e.len as usize];
        f.read_exact(&mut buf)?;
        out.extend_from_slice(&buf);
        Ok(out)
    }
}

// ---------------------------------------------------------------------------------------------
// Source 2 texture (.vtex_c) → RGBA, mip 0 only
// Layout follows ValveResourceFormat (Resource / Texture): resource header with blocks, the
// DATA block is the texture header, pixel data follows the DATA block.
// ---------------------------------------------------------------------------------------------

const FMT_DXT1: u8 = 1;
const FMT_DXT5: u8 = 2;
const FMT_RGBA8888: u8 = 4;
const FMT_BGRA8888: u8 = 28;

fn rd_u16(b: &[u8], p: usize) -> Result<u16, String> {
    b.get(p..p + 2).map(|s| u16::from_le_bytes([s[0], s[1]])).ok_or_else(|| "truncated".into())
}
fn rd_u32(b: &[u8], p: usize) -> Result<u32, String> {
    b.get(p..p + 4).map(|s| u32::from_le_bytes(s.try_into().unwrap())).ok_or_else(|| "truncated".into())
}

fn level_size(fmt: u8, w: usize, h: usize) -> usize {
    match fmt {
        FMT_DXT1 => w.div_ceil(4).max(1) * h.div_ceil(4).max(1) * 8,
        FMT_DXT5 => w.div_ceil(4).max(1) * h.div_ceil(4).max(1) * 16,
        _ => w * h * 4,
    }
}

pub fn decode_vtex(b: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let block_offset = rd_u32(b, 8)? as usize;
    let block_count = rd_u32(b, 12)? as usize;
    let mut data = None;
    for i in 0..block_count.min(32) {
        let p = 8 + block_offset + i * 12;
        if b.get(p..p + 4) == Some(b"DATA") {
            let off = p + 4 + rd_u32(b, p + 4)? as usize;
            let len = rd_u32(b, p + 8)? as usize;
            data = Some((off, len));
        }
    }
    let (d, dlen) = data.ok_or("no DATA block")?;
    let w = rd_u16(b, d + 20)? as usize;
    let h = rd_u16(b, d + 22)? as usize;
    let depth = rd_u16(b, d + 24)?;
    let fmt = *b.get(d + 26).ok_or("truncated")?;
    let mips = (*b.get(d + 27).ok_or("truncated")?).max(1) as usize;
    if w == 0 || h == 0 || w > 8192 || h > 8192 || depth > 1 {
        return Err(format!("unsupported size {w}x{h}x{depth}"));
    }
    if !matches!(fmt, FMT_DXT1 | FMT_DXT5 | FMT_RGBA8888 | FMT_BGRA8888) {
        return Err(format!("unsupported texture format {fmt}"));
    }
    // Extra data: type 4 = compressed (LZ4) mip sizes, indexed by mip level (0 = largest).
    let extra_off = d + 28 + 4 + rd_u32(b, d + 28 + 4)? as usize;
    let extra_count = rd_u32(b, d + 28 + 8)? as usize;
    let mut compressed: Option<Vec<usize>> = None;
    for j in 0..extra_count.min(16) {
        let e = extra_off + j * 12;
        let kind = rd_u32(b, e)?;
        let at = e + 4 + rd_u32(b, e + 4)? as usize;
        if kind == 4 {
            let is_compressed = rd_u32(b, at)? != 0;
            let n = rd_u32(b, at + 8)? as usize;
            if is_compressed && n == mips {
                compressed = Some((0..n).map(|k| rd_u32(b, at + 12 + k * 4).map(|v| v as usize)).collect::<Result<_, _>>()?);
            }
        }
    }
    // Mips are stored smallest first; mip 0 is last.
    let pixels_start = d + dlen;
    let level0 = level_size(fmt, w, h);
    let raw: Vec<u8> = match compressed {
        Some(sizes) => {
            let skip: usize = sizes[1..].iter().sum();
            let src = b.get(pixels_start + skip..pixels_start + skip + sizes[0]).ok_or("truncated pixels")?;
            if sizes[0] == level0 {
                src.to_vec()
            } else {
                lz4_block_decompress(src, level0)?
            }
        }
        None => {
            let skip: usize = (1..mips).map(|m| level_size(fmt, (w >> m).max(1), (h >> m).max(1))).sum();
            b.get(pixels_start + skip..pixels_start + skip + level0).ok_or("truncated pixels")?.to_vec()
        }
    };
    if raw.len() < level0 {
        return Err("short pixel data".into());
    }
    let rgba = match fmt {
        FMT_BGRA8888 => {
            let mut v = raw[..level0].to_vec();
            for px in v.chunks_exact_mut(4) {
                px.swap(0, 2);
            }
            v
        }
        FMT_RGBA8888 => raw[..level0].to_vec(),
        _ => decode_bc(&raw, w, h, fmt == FMT_DXT5),
    };
    Ok((w as u32, h as u32, rgba))
}

/// LZ4 block format (no frame header).
pub fn lz4_block_decompress(src: &[u8], expected: usize) -> Result<Vec<u8>, String> {
    let mut out: Vec<u8> = Vec::with_capacity(expected);
    let mut i = 0;
    let bad = || "corrupt LZ4 data".to_string();
    while i < src.len() {
        let token = src[i];
        i += 1;
        let mut lit = (token >> 4) as usize;
        if lit == 15 {
            loop {
                let b = *src.get(i).ok_or_else(bad)?;
                i += 1;
                lit += b as usize;
                if b != 255 {
                    break;
                }
            }
        }
        out.extend_from_slice(src.get(i..i + lit).ok_or_else(bad)?);
        i += lit;
        if i >= src.len() {
            break;
        }
        let off = u16::from_le_bytes([src[i], *src.get(i + 1).ok_or_else(bad)?]) as usize;
        i += 2;
        let mut m = (token & 15) as usize;
        if m == 15 {
            loop {
                let b = *src.get(i).ok_or_else(bad)?;
                i += 1;
                m += b as usize;
                if b != 255 {
                    break;
                }
            }
        }
        m += 4;
        if off == 0 || off > out.len() {
            return Err(bad());
        }
        let start = out.len() - off;
        for k in 0..m {
            let v = out[start + k];
            out.push(v);
        }
        if out.len() > expected {
            return Err(bad());
        }
    }
    Ok(out)
}

fn rgb565(c: u16) -> [u8; 3] {
    let r = ((c >> 11) & 31) as u32;
    let g = ((c >> 5) & 63) as u32;
    let b = (c & 31) as u32;
    [(r * 255 / 31) as u8, (g * 255 / 63) as u8, (b * 255 / 31) as u8]
}

/// BC1 (DXT1) / BC3 (DXT5) → RGBA.
fn decode_bc(raw: &[u8], w: usize, h: usize, bc3: bool) -> Vec<u8> {
    let mut out = vec![0u8; w * h * 4];
    let bw = w.div_ceil(4);
    let bs = if bc3 { 16 } else { 8 };
    for by in 0..h.div_ceil(4) {
        for bx in 0..bw {
            let at = (by * bw + bx) * bs;
            let Some(blk) = raw.get(at..at + bs) else { continue };
            let mut alpha = [255u8; 16];
            let color = if bc3 {
                let (a0, a1) = (blk[0] as u32, blk[1] as u32);
                let mut pal = [0u32; 8];
                pal[0] = a0;
                pal[1] = a1;
                if a0 > a1 {
                    for k in 1..7 {
                        pal[k + 1] = ((7 - k as u32) * a0 + k as u32 * a1) / 7;
                    }
                } else {
                    for k in 1..5 {
                        pal[k + 1] = ((5 - k as u32) * a0 + k as u32 * a1) / 5;
                    }
                    pal[6] = 0;
                    pal[7] = 255;
                }
                let bits = blk[2..8].iter().rev().fold(0u64, |acc, &x| (acc << 8) | x as u64);
                for (k, a) in alpha.iter_mut().enumerate() {
                    *a = pal[((bits >> (3 * k)) & 7) as usize] as u8;
                }
                &blk[8..16]
            } else {
                blk
            };
            let c0 = u16::from_le_bytes([color[0], color[1]]);
            let c1 = u16::from_le_bytes([color[2], color[3]]);
            let (p0, p1) = (rgb565(c0), rgb565(c1));
            let mix = |a: [u8; 3], b: [u8; 3], wa: u32, wb: u32| -> [u8; 4] {
                let f = |i: usize| ((a[i] as u32 * wa + b[i] as u32 * wb) / (wa + wb)) as u8;
                [f(0), f(1), f(2), 255]
            };
            let pal: [[u8; 4]; 4] = if c0 > c1 || bc3 {
                [[p0[0], p0[1], p0[2], 255], [p1[0], p1[1], p1[2], 255], mix(p0, p1, 2, 1), mix(p0, p1, 1, 2)]
            } else {
                [[p0[0], p0[1], p0[2], 255], [p1[0], p1[1], p1[2], 255], mix(p0, p1, 1, 1), [0, 0, 0, 0]]
            };
            let idx = u32::from_le_bytes(color[4..8].try_into().unwrap());
            for k in 0..16 {
                let (px, py) = (bx * 4 + k % 4, by * 4 + k / 4);
                if px >= w || py >= h {
                    continue;
                }
                let mut c = pal[((idx >> (2 * k)) & 3) as usize];
                if bc3 {
                    c[3] = alpha[k];
                }
                out[(py * w + px) * 4..(py * w + px) * 4 + 4].copy_from_slice(&c);
            }
        }
    }
    out
}

pub fn encode_png(w: u32, h: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = vec![];
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header().map_err(|e| e.to_string())?;
        wr.write_image_data(rgba).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_parse() {
        let t = r#"// comment
"de_nuke"
{
	"material"	"overviews/de_nuke" // texture
	"pos_x"		"-3453"	// upper left world coordinate
	"pos_y"		"2887"
	"scale"		"7"
	"verticalsections"
	{
		"default" { "AltitudeMax" "10000" "AltitudeMin" "-495" }
		"lower" { "AltitudeMax" "-495" "AltitudeMin" "-10000" }
	}
}"#;
        let c = parse_overview(t).unwrap();
        assert_eq!(c, Calibration { pos_x: -3453.0, pos_y: 2887.0, scale: 7.0, lower_max_z: Some(-495.0) });
        assert_eq!(parse_overview("\"x\" { \"scale\" \"0\" \"pos_x\" \"1\" \"pos_y\" \"1\" }"), None);
    }

    #[test]
    fn lz4_roundtrip_literal_and_match() {
        // "abcabcabcabc": 3 literals then a match of 9 at offset 3.
        let src = [0x35, b'a', b'b', b'c', 3, 0];
        assert_eq!(lz4_block_decompress(&src, 12).unwrap(), b"abcabcabcabc");
        assert!(lz4_block_decompress(&[0x10, b'a', 9, 0], 10).is_err(), "offset beyond output");
    }

    #[test]
    fn sanitize() {
        assert_eq!(sanitize_map("De_Dust2").as_deref(), Some("de_dust2"));
        assert_eq!(sanitize_map("../x"), None);
        assert_eq!(sanitize_map(""), None);
    }

    #[test]
    fn bc1_solid_block() {
        // c0 = white, c1 = black, all indices 0 → white.
        let blk = [0xff, 0xff, 0, 0, 0, 0, 0, 0];
        let out = decode_bc(&blk, 4, 4, false);
        assert!(out.chunks(4).all(|p| p == [255, 255, 255, 255]));
    }

    /// Real CS2 install, if present: decode de_dust2's radar and check calibration.
    #[test]
    fn cs2_install_radar() {
        let Some(csgo) = locate_cs2(std::env::var("CS_ANALYST_CS2_DIR").ok().as_deref()) else {
            eprintln!("skip: CS2 install not found");
            return;
        };
        let root = std::env::temp_dir().join(format!("cs-analyst-radar-{}", uuid::Uuid::new_v4()));
        let c = ensure_cs2_cache(&root, &csgo, "de_dust2").unwrap();
        assert_eq!(c.calibration, builtin("de_dust2"));
        assert_eq!(c.levels, vec!["default".to_string()]);
        let png = fs::read(cache_dir(&root).join("de_dust2.png")).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        // Second call hits the stamp cache.
        assert_eq!(ensure_cs2_cache(&root, &csgo, "de_dust2").unwrap().levels.len(), 1);
        let nuke = ensure_cs2_cache(&root, &csgo, "de_nuke").unwrap();
        assert_eq!(nuke.levels, vec!["default".to_string(), "lower".to_string()]);
        assert_eq!(nuke.calibration.unwrap().lower_max_z, Some(-495.0));
        let _ = fs::remove_dir_all(root);
    }
}
