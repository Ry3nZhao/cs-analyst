import { createHash } from 'node:crypto';
import { basename } from 'node:path';
import { access, readFile, type FileHandle, open } from 'node:fs/promises';
import { uncompress } from 'snappyjs';
import { fromBinary, CDataGCCStrike15_v2_MatchInfoSchema } from 'csgo-protobuf';

/** Platform hint from GCPD filename strong signal only */
export type DetectedPlatform = 'valve' | 'imported';

export interface ValveFilenameParts {
  gameId: string;
  matchPart: string;
  regionId: string;
}

/** Parsed CDemoFileHeader (1-12) + CDemoFileInfo (1-4) + derived fields */
export interface DemHeader {
  // Derived / legacy
  matchId?: string;
  mapName?: string;
  durationSeconds?: number;
  gameMode?: string;
  gameType?: number;
  playedAt?: string;
  sha256: string;
  isValid: boolean;
  detectedPlatform: DetectedPlatform;
  valveFilenameParts?: ValveFilenameParts;
  // CDemoFileHeader
  demoFileStamp?: string;
  networkProtocol?: number;
  serverName?: string;
  clientName?: string;
  gameDirectory?: string;
  fullpacketsVersion?: number;
  allowClientsideEntities?: boolean;
  demoHeaderField9?: number;
  demoHeaderField10?: string;
  demoVersionName?: string;
  demoVersionGuid?: string;
  // CDemoFileInfo
  playbackTime?: number;
  playbackTicks?: number;
  playbackFrames?: number;
  /** CGameInfo message (field 4) as JSON */
  demGameInfo?: Record<string, unknown> | null;
}

const MAGIC = Buffer.from('PBDEMS2\0');
const DEMO_HEADER_SIZE = 16;
const DEM_IS_COMPRESSED = 64;
const DEM_FileHeader = 1;
const DEM_FileInfo = 2;

/** GCPD / Steam client demo: match{gameId}_{a}_{b}_{region}(_{mode})?.dem */
const VALVE_GCPD_FILENAME =
  /^match(\d+)_(\d+)_(\d+)_(\d+)(?:_(casual|competitive|wingman|premier|deathmatch|armsrace|demolition|retakes))?\.dem$/i;

export function parseValveGcpdFilename(fileName: string): ValveFilenameParts | null {
  const base = basename(fileName);
  const m = base.match(VALVE_GCPD_FILENAME);
  if (!m) return null;
  return { gameId: m[1]!, matchPart: `${m[2]}_${m[3]}`, regionId: m[4]! };
}

export function detectPlatformFromFileName(fileName: string): DetectedPlatform {
  return parseValveGcpdFilename(fileName) ? 'valve' : 'imported';
}

function readVarint(buf: Buffer, offset: number): [bigint, number] {
  let result = 0n;
  let shift = 0n;
  let pos = offset;
  while (pos < buf.length) {
    const byte = buf[pos++]!;
    result |= BigInt(byte & 0x7f) << shift;
    if ((byte & 0x80) === 0) return [result, pos];
    shift += 7n;
  }
  return [result, pos];
}

interface ProtoField {
  num: number;
  wireType: number;
  data: Buffer | bigint | number;
}

export function parseMessage(buf: Buffer): ProtoField[] {
  const fields: ProtoField[] = [];
  let pos = 0;
  while (pos < buf.length) {
    let tag: bigint;
    [tag, pos] = readVarint(buf, pos);
    if (tag === 0n) break;
    const wireType = Number(tag & 7n);
    const num = Number(tag >> 3n);
    switch (wireType) {
      case 0: {
        let val: bigint;
        [val, pos] = readVarint(buf, pos);
        fields.push({ num, wireType, data: val });
        break;
      }
      case 1: {
        const lo = BigInt(buf.readUInt32LE(pos));
        const hi = BigInt(buf.readUInt32LE(pos + 4));
        pos += 8;
        fields.push({ num, wireType, data: (hi << 32n) | lo });
        break;
      }
      case 2: {
        let len: bigint;
        [len, pos] = readVarint(buf, pos);
        const n = Number(len);
        const data = buf.subarray(pos, pos + n);
        pos += n;
        fields.push({ num, wireType, data });
        break;
      }
      case 5: {
        const val = buf.readFloatLE(pos);
        pos += 4;
        fields.push({ num, wireType, data: val });
        break;
      }
      default:
        pos = buf.length;
    }
  }
  return fields;
}

function getField(fields: ProtoField[], num: number): ProtoField | undefined {
  return fields.find((f) => f.num === num);
}

function getSubMessage(fields: ProtoField[], num: number): ProtoField[] | undefined {
  const f = getField(fields, num);
  if (f && f.wireType === 2 && Buffer.isBuffer(f.data)) {
    return parseMessage(f.data);
  }
  return undefined;
}

function getString(fields: ProtoField[], num: number): string | undefined {
  const f = getField(fields, num);
  if (f && f.wireType === 2 && Buffer.isBuffer(f.data)) {
    if (f.data.length === 0) return '';
    return f.data.toString('utf8');
  }
  return undefined;
}

function getVarint(fields: ProtoField[], num: number): number | undefined {
  const f = getField(fields, num);
  if (f && f.wireType === 0) return Number(f.data as bigint);
  return undefined;
}

function getVarintString(fields: ProtoField[], num: number): string | undefined {
  const f = getField(fields, num);
  if (f && f.wireType === 0) {
    const s = (f.data as bigint).toString();
    return s === '0' ? undefined : s;
  }
  return undefined;
}

/** Parse CSGO nested game info from raw CGameInfo proto fields (preserves uint64 matchId). */
export function parseCsgoFromGameInfoFields(
  gameInfoFields: ProtoField[] | undefined,
): {
  matchId?: string;
  mapName?: string;
  gameType?: number;
  gameMode?: string;
} {
  if (!gameInfoFields) return {};
  const csgoFields = getSubMessage(gameInfoFields, 2);
  if (!csgoFields) return {};

  const matchId = getVarintString(csgoFields, 1);
  const mapName = getString(csgoFields, 2);
  const gameType = getVarint(csgoFields, 3);
  let gameMode: string | undefined;
  const mode = getVarint(csgoFields, 5);
  if (mode != null) gameMode = gameModeLabel(mode);

  return { matchId, mapName, gameType, gameMode };
}

function fieldsToJson(fields: ProtoField[]): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const f of fields) {
    const key = String(f.num);
    if (f.wireType === 0) {
      out[key] = Number(f.data as bigint);
    } else if (f.wireType === 5) {
      out[key] = f.data as number;
    } else if (f.wireType === 2 && Buffer.isBuffer(f.data)) {
      const sub = parseMessage(f.data);
      out[key] = sub.length > 0 ? fieldsToJson(sub) : f.data.toString('utf8');
    } else if (f.wireType === 1) {
      out[key] = (f.data as bigint).toString();
    }
  }
  return out;
}

function parseFileHeader(fields: ProtoField[]) {
  return {
    demoFileStamp: getString(fields, 1),
    networkProtocol: getVarint(fields, 2),
    serverName: getString(fields, 3),
    clientName: getString(fields, 4),
    mapName: getString(fields, 5),
    gameDirectory: getString(fields, 6),
    fullpacketsVersion: getVarint(fields, 7),
    allowClientsideEntities: getVarint(fields, 8) === 1,
    demoHeaderField9: getVarint(fields, 9),
    demoHeaderField10: getString(fields, 10),
    demoVersionName: getString(fields, 11),
    demoVersionGuid: getString(fields, 12),
  };
}

function parseFileInfo(fields: ProtoField[]) {
  const ptField = getField(fields, 1);
  const playbackTime =
    ptField && ptField.wireType === 5 ? (ptField.data as number) : undefined;
  const gameInfoFields = getSubMessage(fields, 4);
  return {
    playbackTime,
    durationSeconds: playbackTime != null ? Math.round(playbackTime) : undefined,
    playbackTicks: getVarint(fields, 2),
    playbackFrames: getVarint(fields, 3),
    demGameInfo: gameInfoFields ? fieldsToJson(gameInfoFields) : null,
    gameInfoFields,
  };
}

async function computeSha256(fh: FileHandle, size: number): Promise<string> {
  const hash = createHash('sha256');
  const chunkSize = 256 * 1024;
  let offset = 0;
  while (offset < size) {
    const toRead = Math.min(chunkSize, size - offset);
    const chunk = Buffer.alloc(toRead);
    await fh.read(chunk, 0, toRead, offset);
    hash.update(chunk);
    offset += toRead;
  }
  return hash.digest('hex');
}

function gameModeLabel(mode: number): string {
  switch (mode) {
    case 1: return 'casual';
    case 2: return 'competitive';
    case 3: return 'wingman';
    case 4: return 'armsrace';
    case 5: return 'demolition';
    case 6: return 'deathmatch';
    case 11: return 'premier';
    case 12: return 'retakes';
    default: return mode > 0 ? String(mode) : 'unknown';
  }
}

function gameModeFromFileName(filePath: string): string | undefined {
  const name = basename(filePath).toLowerCase();
  const gcpd = name.match(VALVE_GCPD_FILENAME);
  if (gcpd?.[5]) return gcpd[5];
  const match = name.match(
    /_(casual|competitive|wingman|premier|deathmatch|armsrace|demolition|retakes)(?:\.\w+)?$/,
  );
  return match?.[1];
}

async function readMatchIdFromInfoFile(demPath: string): Promise<string | undefined> {
  const infoPath = `${demPath}.info`;
  try {
    await access(infoPath);
  } catch {
    return undefined;
  }
  try {
    const bytes = await readFile(infoPath);
    const msg = fromBinary(CDataGCCStrike15_v2_MatchInfoSchema, new Uint8Array(bytes));
    const id = msg.matchid;
    if (id != null && id > 0n) {
      return id.toString();
    }
  } catch {
    return undefined;
  }
  return undefined;
}

function parseCsgoFromGameInfo(demGameInfo: Record<string, unknown> | null | undefined) {
  if (!demGameInfo) return {};
  const csgo = demGameInfo['2'] as Record<string, unknown> | undefined;
  if (!csgo || typeof csgo !== 'object') return {};

  let matchId: string | undefined;
  const mid = csgo['1'];
  if (typeof mid === 'string' && mid !== '0') matchId = mid;
  else if (typeof mid === 'number' && mid > 0) matchId = String(mid);

  const mapName = typeof csgo['2'] === 'string' && csgo['2'] ? csgo['2'] : undefined;
  const gameType = typeof csgo['3'] === 'number' ? csgo['3'] : undefined;
  let gameMode: string | undefined;
  if (typeof csgo['5'] === 'number') gameMode = gameModeLabel(csgo['5']);

  return { matchId, mapName, gameType, gameMode };
}

/** @deprecated prefer parseCsgoFromGameInfoFields for matchId precision */

async function readDemoCommand(
  fh: FileHandle,
  fileOffset: number,
): Promise<{ cmd: number; body: Buffer } | null> {
  const hdrChunk = Buffer.alloc(64);
  const { bytesRead } = await fh.read(hdrChunk, 0, 64, fileOffset);
  if (bytesRead === 0) return null;

  let pos = 0;
  let rawCmd: bigint;
  [rawCmd, pos] = readVarint(hdrChunk, pos);
  const compressed = (Number(rawCmd) & DEM_IS_COMPRESSED) !== 0;
  const cmd = Number(rawCmd) & ~DEM_IS_COMPRESSED;

  [, pos] = readVarint(hdrChunk, pos);

  let bodySize: bigint;
  [bodySize, pos] = readVarint(hdrChunk, pos);
  const headerSize = pos;
  const size = Number(bodySize);
  if (size < 0 || size > 64 * 1024 * 1024) return null;

  const body = Buffer.alloc(size);
  await fh.read(body, 0, size, fileOffset + headerSize);

  const payload = compressed ? Buffer.from(uncompress(body)) : body;
  return { cmd, body: payload };
}

export async function analyzeDem(filePath: string): Promise<DemHeader> {
  const fileName = basename(filePath);
  const valveFilenameParts = parseValveGcpdFilename(fileName) ?? undefined;
  const detectedPlatform = valveFilenameParts ? 'valve' : 'imported';

  const fh = await open(filePath, 'r');
  try {
    const headerBuf = Buffer.alloc(DEMO_HEADER_SIZE);
    await fh.read(headerBuf, 0, DEMO_HEADER_SIZE, 0);

    if (!headerBuf.subarray(0, 8).equals(MAGIC)) {
      return { sha256: '', isValid: false, demGameInfo: null, detectedPlatform, valveFilenameParts };
    }

    const fileInfoOffset = headerBuf.readUInt32LE(8);
    const stat = await fh.stat();
    const sha256 = await computeSha256(fh, stat.size);

    let fileHeader = parseFileHeader([]);
    let fileInfo = parseFileInfo([]);
    let gameInfoFields: ProtoField[] | undefined;

    const fileHeaderCmd = await readDemoCommand(fh, DEMO_HEADER_SIZE);
    if (fileHeaderCmd?.cmd === DEM_FileHeader) {
      fileHeader = parseFileHeader(parseMessage(fileHeaderCmd.body));
    }

    const fileInfoCmd = await readDemoCommand(fh, fileInfoOffset);
    if (fileInfoCmd?.cmd === DEM_FileInfo) {
      const fileInfoMsg = parseMessage(fileInfoCmd.body);
      fileInfo = parseFileInfo(fileInfoMsg);
      gameInfoFields = fileInfo.gameInfoFields;
    }

    const csgoMeta = parseCsgoFromGameInfoFields(gameInfoFields);
    let mapName = csgoMeta.mapName ?? fileHeader.mapName;
    let matchId = csgoMeta.matchId ?? (await readMatchIdFromInfoFile(filePath));
    let gameMode = csgoMeta.gameMode;
    let gameType = csgoMeta.gameType;

    gameMode = gameMode ?? gameModeFromFileName(filePath);
    if (gameType == null && basename(filePath).toLowerCase().startsWith('match730_')) {
      gameType = 730;
    }

    return {
      matchId,
      mapName,
      durationSeconds: fileInfo.durationSeconds,
      gameMode,
      gameType,
      playedAt: undefined,
      sha256,
      isValid: true,
      detectedPlatform,
      valveFilenameParts,
      demoFileStamp: fileHeader.demoFileStamp,
      networkProtocol: fileHeader.networkProtocol,
      serverName: fileHeader.serverName,
      clientName: fileHeader.clientName,
      gameDirectory: fileHeader.gameDirectory,
      fullpacketsVersion: fileHeader.fullpacketsVersion,
      allowClientsideEntities: fileHeader.allowClientsideEntities,
      demoHeaderField9: fileHeader.demoHeaderField9,
      demoHeaderField10: fileHeader.demoHeaderField10,
      demoVersionName: fileHeader.demoVersionName,
      demoVersionGuid: fileHeader.demoVersionGuid,
      playbackTime: fileInfo.playbackTime,
      playbackTicks: fileInfo.playbackTicks,
      playbackFrames: fileInfo.playbackFrames,
      demGameInfo: fileInfo.demGameInfo,
    };
  } finally {
    await fh.close();
  }
}
