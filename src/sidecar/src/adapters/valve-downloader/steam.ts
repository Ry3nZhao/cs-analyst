/**
 * Adapted from cs-demo-downloader steam.ts (MIT)
 * https://github.com/claabs/cs-demo-downloader
 */
import SteamTotp from 'steam-totp';
import SteamUser from 'steam-user';
import promiseTimeout from 'p-timeout';
import type { BotLogin } from './types.js';

export interface SteamClientLoginResult {
  client: SteamUser;
  refreshToken?: string;
}

export async function loginSteamClient(user: BotLogin): Promise<SteamClientLoginResult> {
  const steamUser = new SteamUser();
  let newRefreshToken: string | undefined;

  const waitForAuthentication = promiseTimeout(
    new Promise<void>((resolve) => {
      steamUser.once('loggedOn', () => resolve());
    }),
    { milliseconds: 30000, message: 'Timed out waiting for Steam logged on' },
  );

  if (user.refreshToken) {
    steamUser.logOn({ refreshToken: user.refreshToken });
    await waitForAuthentication;
  } else {
    if (!user.password) {
      throw new Error('Valve bot password is required when no refresh token is stored');
    }
    const authCode = user.secret ? SteamTotp.getAuthCode(user.secret) : undefined;

    const waitForRefreshToken = promiseTimeout(
      new Promise<string>((resolve) => {
        steamUser.once('refreshToken', (token: string) => resolve(token));
      }),
      { milliseconds: 30000, message: 'Timed out waiting for Steam refresh token' },
    );

    steamUser.logOn({
      accountName: user.username,
      password: user.password,
      twoFactorCode: authCode,
    });
    await waitForAuthentication;
    newRefreshToken = await waitForRefreshToken;
  }

  return { client: steamUser, refreshToken: newRefreshToken };
}
