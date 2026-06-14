import fs from 'node:fs';
import fsp from 'node:fs/promises';
import stream from 'node:stream';
import util from 'node:util';
import axios from 'axios';
import bz2 from 'unbzip2-stream';
import GlobalOffensive from 'globaloffensive';
import promiseTimeout from 'p-timeout';
import SteamUser from 'steam-user';
import { gcpdUrlToFilename } from './discover.js';

const pipeline = util.promisify(stream.pipeline);

export async function downloadValveDemo(params: {
  outputPath: string;
  demoUrl?: string | null;
  shareCode?: string | null;
  botUsername?: string;
  botPassword?: string;
  botSecret?: string;
}): Promise<{ checksum?: string }> {
  let demoUrl = params.demoUrl;

  if (!demoUrl && params.shareCode && params.botUsername && params.botPassword) {
    demoUrl = await resolveDemoUrlFromShareCode({
      shareCode: params.shareCode,
      botUsername: params.botUsername,
      botPassword: params.botPassword,
      botSecret: params.botSecret,
    });
  }

  if (!demoUrl) throw new Error('No demo URL available');

  if (demoUrl.endsWith('.bz2')) {
    await downloadBz2(demoUrl, params.outputPath);
  } else {
    const resp = await axios.get(demoUrl, { responseType: 'arraybuffer' });
    await fsp.writeFile(params.outputPath, Buffer.from(resp.data));
  }

  const stat = await fsp.stat(params.outputPath);
  return { checksum: `${stat.size}` };
}

async function downloadBz2(url: string, outputPath: string): Promise<void> {
  const tempPath = `${outputPath}.tmp`;
  const resp = await axios.get<stream.Duplex>(url, { responseType: 'stream' });
  await pipeline(resp.data, bz2(), fs.createWriteStream(tempPath, { flags: 'w' }));
  await fsp.rename(tempPath, outputPath);
}

async function resolveDemoUrlFromShareCode(params: {
  shareCode: string;
  botUsername: string;
  botPassword: string;
  botSecret?: string;
}): Promise<string> {
  const client = new SteamUser();
  await new Promise<void>((resolve, reject) => {
    client.logOn({
      accountName: params.botUsername,
      password: params.botPassword,
      twoFactorCode: params.botSecret,
    });
    client.once('loggedOn', () => resolve());
    client.once('error', reject);
  });

  client.gamesPlayed(730, true);
  await promiseTimeout(
    new Promise<void>((resolve) => {
      client.once('appLaunched', (id: number) => {
        if (id === 730) resolve();
      });
    }),
    { milliseconds: 30000, message: 'Timed out waiting for CS2 launch' },
  );

  const csgo = new GlobalOffensive(client);
  const { decodeMatchShareCode } = await import('csgo-sharecode');
  const { matchId } = decodeMatchShareCode(params.shareCode);

  const match = await promiseTimeout(
    new Promise<{ demo_url?: string }>((resolve, reject) => {
      const onList = (matches: Array<{ matchid: string; demo_url?: string }>) => {
        const found = matches.find((m) => m.matchid === matchId.toString());
        if (found) {
          csgo.removeListener('matchList', onList);
          resolve(found);
        }
      };
      csgo.on('matchList', onList);
      csgo.requestGame(matchId.toString());
      setTimeout(() => reject(new Error('Match metadata timeout')), 30000);
    }),
    { milliseconds: 35000 },
  );

  client.logOff();
  if (!match.demo_url) throw new Error('URL_EXPIRED');
  return match.demo_url;
}

export { gcpdUrlToFilename };
