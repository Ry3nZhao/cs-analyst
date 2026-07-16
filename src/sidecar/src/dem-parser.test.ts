import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  parseCsgoFromGameInfoFields,
  parseMessage,
  parseValveGcpdFilename,
  detectPlatformFromFileName,
} from './dem-parser.js';

function encodeVarint(n: bigint): Buffer {
  const bytes: number[] = [];
  let value = n;
  while (value >= 0x80n) {
    bytes.push(Number((value & 0x7fn) | 0x80n));
    value >>= 7n;
  }
  bytes.push(Number(value));
  return Buffer.from(bytes);
}

function fieldVarint(fieldNum: number, value: bigint): Buffer {
  const tag = encodeVarint(BigInt((fieldNum << 3) | 0));
  return Buffer.concat([tag, encodeVarint(value)]);
}

function fieldBytes(fieldNum: number, data: Buffer): Buffer {
  const tag = encodeVarint(BigInt((fieldNum << 3) | 2));
  const len = encodeVarint(BigInt(data.length));
  return Buffer.concat([tag, len, data]);
}

describe('parseCsgoFromGameInfoFields', () => {
  it('preserves uint64 matchId without precision loss', () => {
    const matchId = 3827992342472688030n;
    const csgo = fieldVarint(1, matchId);
    const gameInfo = fieldBytes(2, csgo);
    const fields = parseMessage(gameInfo);
    const parsed = parseCsgoFromGameInfoFields(fields);
    assert.equal(parsed.matchId, '3827992342472688030');
  });
});

describe('parseValveGcpdFilename', () => {
  it('recognizes GCPD names with optional game mode suffix', () => {
    const name = 'match730_003819551067436220852_0521272874_401_competitive.dem';
    assert.deepEqual(parseValveGcpdFilename(name), {
      gameId: '730',
      matchPart: '003819551067436220852_0521272874',
      regionId: '401',
    });
    assert.equal(detectPlatformFromFileName(name), 'valve');
  });
});
