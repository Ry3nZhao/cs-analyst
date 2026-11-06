//! 只读 demo 文件头的快速路径（不跑完整解析）：`CDemoFileHeader`（1–12）、`CDemoFileInfo`（1–4）、
//! 文件 sha256，以及由文件名 / `.dem.info` 推导出的 matchId、游戏模式、平台。
//!
//! 取代原来下载模块里的 `dem-parser.ts`，字段语义与它逐项一致（包括 `demGameInfo` 的 JSON 形状），
//! 这样迁移前后 `matches` 表里的元数据不变。与旧实现的唯一差别：嵌套字段里有些字符串碰巧像 protobuf，
//! 旧实现会越界抛错导致整个文件头读取失败；这里把这种情况当成字符串处理。

use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const MAGIC: &[u8; 8] = b"PBDEMS2\0";
const DEMO_HEADER_SIZE: u64 = 16;
const DEM_IS_COMPRESSED: u64 = 64;
const DEM_FILE_HEADER: u64 = 1;
const DEM_FILE_INFO: u64 = 2;
const MAX_COMMAND_SIZE: u64 = 64 * 1024 * 1024;

const GAME_MODES: &[&str] = &[
    "casual",
    "competitive",
    "wingman",
    "premier",
    "deathmatch",
    "armsrace",
    "demolition",
    "retakes",
];

#[derive(Debug, thiserror::Error)]
pub enum HeaderError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("snappy: {0}")]
    Snappy(#[from] snap::Error),
    #[error("malformed protobuf in demo header")]
    Malformed,
}

/// 文件头元数据。字符串字段保留原值（可能是空字符串），由调用方决定是否把空串当成缺失。
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DemoFileMeta {
    pub match_id: Option<String>,
    pub map_name: Option<String>,
    pub duration_seconds: Option<i64>,
    pub game_mode: Option<String>,
    pub game_type: Option<i64>,
    /// 文件头里没有比赛时间；保留字段以便调用方统一处理（目前恒为 `None`）。
    pub played_at: Option<String>,
    /// 整个文件的 sha256（小写十六进制）；文件头无效时为空串。
    pub sha256: String,
    pub is_valid: bool,
    /// `valve`（GCPD 文件名）或 `imported`。
    pub detected_platform: String,
    // CDemoFileHeader
    pub demo_file_stamp: Option<String>,
    pub network_protocol: Option<i64>,
    pub server_name: Option<String>,
    pub client_name: Option<String>,
    pub game_directory: Option<String>,
    pub fullpackets_version: Option<i64>,
    pub allow_clientside_entities: Option<bool>,
    pub demo_header_field_9: Option<i64>,
    pub demo_header_field_10: Option<String>,
    pub demo_version_name: Option<String>,
    pub demo_version_guid: Option<String>,
    // CDemoFileInfo
    pub playback_time: Option<f64>,
    pub playback_ticks: Option<i64>,
    pub playback_frames: Option<i64>,
    /// `CGameInfo`（CDemoFileInfo 字段 4）按字段号转成的 JSON；没有该字段时为 `None`。
    pub dem_game_info: Option<Value>,
}

/// 读 demo 文件头 + 计算 sha256。不是 CS2 demo（魔数不对）时返回 `is_valid = false`，不报错。
pub fn read_demo_file_meta(path: &Path) -> Result<DemoFileMeta, HeaderError> {
    let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let gcpd = parse_valve_gcpd_filename(&file_name);
    let detected_platform = if gcpd.is_some() { "valve" } else { "imported" }.to_string();

    let mut file = File::open(path)?;
    let mut head = [0u8; DEMO_HEADER_SIZE as usize];
    read_at(&mut file, 0, &mut head)?;
    if &head[0..8] != MAGIC {
        return Ok(DemoFileMeta { is_valid: false, detected_platform, ..Default::default() });
    }
    let file_info_offset = u32::from_le_bytes([head[8], head[9], head[10], head[11]]) as u64;
    let sha256 = sha256_file(&mut file)?;

    let mut meta = DemoFileMeta { sha256, is_valid: true, detected_platform, ..Default::default() };

    let mut header_map_name = None;
    let header_body = match read_demo_command(&mut file, DEMO_HEADER_SIZE)? {
        Some((cmd, body)) if cmd == DEM_FILE_HEADER => body,
        _ => Vec::new(),
    };
    {
        let f = parse_message(&header_body).map_err(|_| HeaderError::Malformed)?;
        meta.demo_file_stamp = get_string(&f, 1);
        meta.network_protocol = get_int(&f, 2);
        meta.server_name = get_string(&f, 3);
        meta.client_name = get_string(&f, 4);
        header_map_name = header_map_name.or(get_string(&f, 5));
        meta.game_directory = get_string(&f, 6);
        meta.fullpackets_version = get_int(&f, 7);
        meta.allow_clientside_entities = Some(get_varint(&f, 8).map(js_number) == Some(1.0));
        meta.demo_header_field_9 = get_int(&f, 9);
        meta.demo_header_field_10 = get_string(&f, 10);
        meta.demo_version_name = get_string(&f, 11);
        meta.demo_version_guid = get_string(&f, 12);
    }

    let mut csgo = CsgoMeta::default();
    if let Some((cmd, body)) = read_demo_command(&mut file, file_info_offset)? {
        if cmd == DEM_FILE_INFO {
            let f = parse_message(&body).map_err(|_| HeaderError::Malformed)?;
            if let Some(field) = get_field(&f, 1) {
                if let FieldData::Fixed32(x) = field.data {
                    let t = x as f64;
                    meta.playback_time = js_value(t).as_f64();
                    meta.duration_seconds = js_value((t + 0.5).floor()).as_i64();
                }
            }
            meta.playback_ticks = get_int(&f, 2);
            meta.playback_frames = get_int(&f, 3);
            if let Some(game_info) = get_bytes(&f, 4) {
                let gi = parse_message(game_info).map_err(|_| HeaderError::Malformed)?;
                meta.dem_game_info = Some(fields_to_json(&gi));
                csgo = csgo_from_game_info(&gi)?;
            }
        }
    }

    meta.map_name = csgo.map_name.or(header_map_name);
    meta.match_id = csgo.match_id.or_else(|| read_match_id_from_info_file(path));
    meta.game_type = csgo.game_type;
    meta.game_mode = csgo.game_mode.or_else(|| game_mode_from_file_name(&file_name));
    if meta.game_type.is_none() && file_name.to_ascii_lowercase().starts_with("match730_") {
        meta.game_type = Some(730);
    }
    Ok(meta)
}

/// GCPD / Steam 客户端 demo 文件名：`match{gameId}_{a}_{b}_{region}(_{mode})?.dem`（不区分大小写）。
/// 返回 `(gameId, "a_b", region)`。
pub fn parse_valve_gcpd_filename(file_name: &str) -> Option<(String, String, String)> {
    let lower = file_name.to_ascii_lowercase();
    let stem = lower.strip_suffix(".dem")?.strip_prefix("match")?;
    let parts: Vec<&str> = stem.split('_').collect();
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let ok = match parts.len() {
        4 => true,
        5 => GAME_MODES.contains(&parts[4]),
        _ => false,
    };
    if !ok || !parts[..4].iter().all(|p| digits(p)) {
        return None;
    }
    Some((parts[0].to_string(), format!("{}_{}", parts[1], parts[2]), parts[3].to_string()))
}

#[derive(Default)]
struct CsgoMeta {
    match_id: Option<String>,
    map_name: Option<String>,
    game_type: Option<i64>,
    game_mode: Option<String>,
}

fn csgo_from_game_info(game_info: &[Field<'_>]) -> Result<CsgoMeta, HeaderError> {
    let Some(bytes) = get_bytes(game_info, 2) else { return Ok(CsgoMeta::default()) };
    let f = parse_message(bytes).map_err(|_| HeaderError::Malformed)?;
    let match_id = match get_field(&f, 1).map(|x| &x.data) {
        Some(FieldData::Varint(v)) if *v != 0 => Some(v.to_string()),
        _ => None,
    };
    let game_mode = get_varint(&f, 5).map(|m| game_mode_label(js_number(m)));
    Ok(CsgoMeta { match_id, map_name: get_string(&f, 2), game_type: get_int(&f, 3), game_mode })
}

fn game_mode_label(mode: f64) -> String {
    match mode {
        m if m == 1.0 => "casual".into(),
        m if m == 2.0 => "competitive".into(),
        m if m == 3.0 => "wingman".into(),
        m if m == 4.0 => "armsrace".into(),
        m if m == 5.0 => "demolition".into(),
        m if m == 6.0 => "deathmatch".into(),
        m if m == 11.0 => "premier".into(),
        m if m == 12.0 => "retakes".into(),
        m if m > 0.0 => format!("{m}"),
        _ => "unknown".into(),
    }
}

fn game_mode_from_file_name(file_name: &str) -> Option<String> {
    let name = file_name.to_ascii_lowercase();
    if let Some(stem) = name.strip_suffix(".dem") {
        if parse_valve_gcpd_filename(&name).is_some() {
            if let Some(mode) = GAME_MODES.iter().find(|m| stem.ends_with(&format!("_{m}"))) {
                return Some(mode.to_string());
            }
        }
    }
    // `_(mode)(?:\.\w+)?$`
    let without_ext = match name.rfind('.') {
        Some(i)
            if i + 1 < name.len()
                && name[i + 1..].bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') =>
        {
            Some(&name[..i])
        }
        _ => None,
    };
    for candidate in [Some(name.as_str()), without_ext].into_iter().flatten() {
        if let Some(mode) = GAME_MODES.iter().find(|m| candidate.ends_with(&format!("_{m}"))) {
            return Some(mode.to_string());
        }
    }
    None
}

/// `{demo}.info`（`CDataGCCStrike15_v2_MatchInfo`）里的 matchid（字段 1，uint64）。
fn read_match_id_from_info_file(dem_path: &Path) -> Option<String> {
    let mut info = dem_path.as_os_str().to_owned();
    info.push(".info");
    let bytes = std::fs::read(Path::new(&info)).ok()?;
    let fields = parse_message(&bytes).ok()?;
    match fields.iter().find(|f| f.num == 1).map(|f| &f.data) {
        Some(FieldData::Varint(v)) if *v > 0 => Some(v.to_string()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// 文件读取

fn read_at(file: &mut File, offset: u64, buf: &mut [u8]) -> std::io::Result<usize> {
    file.seek(SeekFrom::Start(offset))?;
    let mut filled = 0;
    while filled < buf.len() {
        match file.read(&mut buf[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    Ok(filled)
}

fn sha256_file(file: &mut File) -> std::io::Result<String> {
    file.seek(SeekFrom::Start(0))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// 读一条 demo 命令：varint(cmd | compressed) varint(tick) varint(size) body。
fn read_demo_command(file: &mut File, offset: u64) -> Result<Option<(u64, Vec<u8>)>, HeaderError> {
    let mut chunk = [0u8; 64];
    if read_at(file, offset, &mut chunk)? == 0 {
        return Ok(None);
    }
    let (raw_cmd, pos) = read_varint(&chunk, 0);
    let (_tick, pos) = read_varint(&chunk, pos);
    let (size, pos) = read_varint(&chunk, pos);
    if size > MAX_COMMAND_SIZE as u128 {
        return Ok(None);
    }
    let raw_cmd = raw_cmd as u64;
    let compressed = raw_cmd & DEM_IS_COMPRESSED != 0;
    let cmd = raw_cmd & !DEM_IS_COMPRESSED;
    let mut body = vec![0u8; size as usize];
    read_at(file, offset + pos as u64, &mut body)?;
    if compressed {
        body = snap::raw::Decoder::new().decompress_vec(&body)?;
    }
    Ok(Some((cmd, body)))
}

// ---------------------------------------------------------------------------------------------
// 最小 protobuf 读取（不需要 schema）

enum FieldData<'a> {
    Varint(u128),
    Fixed64(u64),
    Bytes(&'a [u8]),
    Fixed32(f32),
}

struct Field<'a> {
    num: u128,
    data: FieldData<'a>,
}

/// 无界 varint（超过 128 位的部分丢弃）；读到结尾还没结束就返回已读到的值。
fn read_varint(buf: &[u8], mut pos: usize) -> (u128, usize) {
    let mut result: u128 = 0;
    let mut shift: u32 = 0;
    while pos < buf.len() {
        let byte = buf[pos];
        pos += 1;
        if shift < 128 {
            result |= ((byte & 0x7f) as u128).checked_shl(shift).unwrap_or(0);
        }
        if byte & 0x80 == 0 {
            return (result, pos);
        }
        shift += 7;
    }
    (result, pos)
}

/// 宽松解析：遇到 group / 未知 wire type 就停下；定长字段越界返回 `Err`。
fn parse_message(buf: &[u8]) -> Result<Vec<Field<'_>>, ()> {
    let mut fields = Vec::new();
    let mut pos = 0;
    while pos < buf.len() {
        let (tag, next) = read_varint(buf, pos);
        pos = next;
        if tag == 0 {
            break;
        }
        let num = tag >> 3;
        match (tag & 7) as u8 {
            0 => {
                let (v, next) = read_varint(buf, pos);
                pos = next;
                fields.push(Field { num, data: FieldData::Varint(v) });
            }
            1 => {
                let bytes = buf.get(pos..pos + 8).ok_or(())?;
                pos += 8;
                fields.push(Field { num, data: FieldData::Fixed64(u64::from_le_bytes(bytes.try_into().unwrap())) });
            }
            2 => {
                let (len, next) = read_varint(buf, pos);
                pos = next;
                let end = (pos as u128).saturating_add(len).min(buf.len() as u128) as usize;
                fields.push(Field { num, data: FieldData::Bytes(&buf[pos..end]) });
                pos = (pos as u128).saturating_add(len).min(usize::MAX as u128) as usize;
            }
            5 => {
                let bytes = buf.get(pos..pos + 4).ok_or(())?;
                pos += 4;
                fields.push(Field { num, data: FieldData::Fixed32(f32::from_le_bytes(bytes.try_into().unwrap())) });
            }
            _ => break,
        }
    }
    Ok(fields)
}

/// 第一个字段号为 `num` 的字段（不看 wire type，与旧实现一致）。
fn get_field<'f, 'a>(fields: &'f [Field<'a>], num: u128) -> Option<&'f Field<'a>> {
    fields.iter().find(|f| f.num == num)
}

fn get_bytes<'a>(fields: &[Field<'a>], num: u128) -> Option<&'a [u8]> {
    match get_field(fields, num)?.data {
        FieldData::Bytes(b) => Some(b),
        _ => None,
    }
}

fn get_string(fields: &[Field<'_>], num: u128) -> Option<String> {
    get_bytes(fields, num).map(|b| String::from_utf8_lossy(b).into_owned())
}

fn get_varint(fields: &[Field<'_>], num: u128) -> Option<u128> {
    match get_field(fields, num)?.data {
        FieldData::Varint(v) => Some(v),
        _ => None,
    }
}

fn get_int(fields: &[Field<'_>], num: u128) -> Option<i64> {
    get_varint(fields, num).and_then(|v| js_value(js_number(v)).as_i64())
}

/// JS `Number(bigint)`：就近舍入到 f64。
fn js_number(v: u128) -> f64 {
    v as f64
}

/// 一个 JS number 经 `JSON.stringify` 再被 serde_json 解析后的值（旧实现的数据就是这样进入 Rust 的）：
/// 非有限值 → null；整数值 → u64 / i64（超出范围则 f64）；其余 → f64。
fn js_value(x: f64) -> Value {
    if !x.is_finite() {
        return Value::Null;
    }
    let text = if x == 0.0 { "0".to_string() } else { format!("{x}") };
    serde_json::from_str(&text).unwrap_or(Value::Null)
}

/// `CGameInfo` → `{ "字段号": 值 }`：varint / fixed32 → 数字，fixed64 → 十进制字符串，
/// bytes → 能解析出字段就递归成对象，否则按 UTF-8 字符串。同号字段后者覆盖前者。
fn fields_to_json(fields: &[Field<'_>]) -> Value {
    let mut by_num: BTreeMap<u128, Value> = BTreeMap::new();
    for f in fields {
        let value = match f.data {
            FieldData::Varint(v) => js_value(js_number(v)),
            FieldData::Fixed32(x) => js_value(x as f64),
            FieldData::Fixed64(v) => Value::String(v.to_string()),
            FieldData::Bytes(b) => bytes_to_json(b),
        };
        by_num.insert(f.num, value);
    }
    let mut out = Map::new();
    for (num, value) in by_num {
        out.insert(num.to_string(), value);
    }
    Value::Object(out)
}

/// bytes 字段当成子消息的条件：自身能解析出字段，且它的直接 bytes 子字段也都能解析（否则多半是
/// 碰巧像 protobuf 的字符串，例如 "Rien"）。旧实现遇到后一种情况会抛错，正常数据两者结果相同。
fn bytes_to_json(b: &[u8]) -> Value {
    if let Ok(sub) = parse_message(b) {
        let children_ok = sub.iter().all(|f| match f.data {
            FieldData::Bytes(c) => parse_message(c).is_ok(),
            _ => true,
        });
        if !sub.is_empty() && children_ok {
            return fields_to_json(&sub);
        }
    }
    Value::String(String::from_utf8_lossy(b).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn varint(mut n: u128) -> Vec<u8> {
        let mut out = Vec::new();
        while n >= 0x80 {
            out.push((n as u8 & 0x7f) | 0x80);
            n >>= 7;
        }
        out.push(n as u8);
        out
    }
    fn fv(num: u32, v: u128) -> Vec<u8> {
        [varint((num as u128) << 3), varint(v)].concat()
    }
    fn fb(num: u32, d: &[u8]) -> Vec<u8> {
        [varint(((num as u128) << 3) | 2), varint(d.len() as u128), d.to_vec()].concat()
    }

    #[test]
    fn match_id_keeps_u64_precision() {
        let gi = fb(2, &fv(1, 3827992342472688030));
        let fields = parse_message(&gi).unwrap();
        let csgo = csgo_from_game_info(&fields).unwrap();
        assert_eq!(csgo.match_id.as_deref(), Some("3827992342472688030"));
        // demGameInfo 里的数字与旧实现一致：经过 JS Number 舍入
        assert_eq!(fields_to_json(&fields).to_string(), r#"{"2":{"1":3827992342472688000}}"#);
    }

    #[test]
    fn string_that_looks_like_protobuf_does_not_fail() {
        // "Rien" → 'R' = 字段 10 / bytes，'i' 当长度时越界；旧实现在这里会抛错
        let gi = fb(10, &[fv(1, 76561198000000042), fb(2, b"Rien")].concat());
        let fields = parse_message(&gi).unwrap();
        let json = fields_to_json(&fields);
        assert_eq!(json["10"]["2"], Value::String("Rien".into()));
    }

    #[test]
    fn gcpd_filename() {
        let name = "match730_003819551067436220852_0521272874_401_competitive.dem";
        assert_eq!(
            parse_valve_gcpd_filename(name),
            Some(("730".into(), "003819551067436220852_0521272874".into(), "401".into()))
        );
        assert!(parse_valve_gcpd_filename("MATCH730_1_2_3.DEM").is_some());
        assert!(parse_valve_gcpd_filename("match730_1_2_3_foo.dem").is_none());
        assert!(parse_valve_gcpd_filename("g161-n-20260708151346440195771.dem").is_none());
        assert_eq!(game_mode_from_file_name(name).as_deref(), Some("competitive"));
        assert_eq!(game_mode_from_file_name("x_premier.dem").as_deref(), Some("premier"));
        assert_eq!(game_mode_from_file_name("x_premier").as_deref(), Some("premier"));
        assert_eq!(game_mode_from_file_name("x.dem"), None);
    }

    #[test]
    fn js_number_formatting() {
        assert_eq!(js_value(0.0).to_string(), "0");
        assert_eq!(js_value(-0.0).to_string(), "0");
        assert_eq!(js_value(1.5).to_string(), "1.5");
        assert_eq!(js_value(-2.0).to_string(), "-2");
        assert_eq!(js_value(9007199254740993u64 as f64).to_string(), "9007199254740992");
        assert_eq!(js_value(f64::NAN), Value::Null);
    }
}
