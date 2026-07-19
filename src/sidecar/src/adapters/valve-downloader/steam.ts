/**
 * Adapted from cs-demo-downloader steam.ts (MIT)
 * https://github.com/claabs/cs-demo-downloader
 */
import type { EventEmitter } from 'node:events';
import SteamTotp from 'steam-totp';
import SteamUser from 'steam-user';
import promiseTimeout from 'p-timeout';
import type { BotLogin } from './types.js';

export interface SteamClientLoginResult {
  client: SteamUser;
  refreshToken?: string;
}

type LogOnDetails = Parameters<SteamUser['logOn']>[0];

function logOnOnce(
  steamUser: SteamUser,
  details: LogOnDetails,
  secret: string | undefined | null,
): Promise<void> {
  const attempt = new Promise<void>((resolve, reject) => {
    let settled = false;
    const finish = (fn: () => void) => {
      if (settled) return;
      settled = true;
      steamUser.removeListener('loggedOn', onLoggedOn);
      steamUser.removeListener('error', onError);
      // steam-user 的类型声明缺少 steamGuard 事件，走 EventEmitter 基类
      (steamUser as EventEmitter).removeListener('steamGuard', onSteamGuard);
      fn();
    };
    const onLoggedOn = () => finish(() => resolve());
    const onError = (err: Error) =>
      finish(() => reject(new Error(`Steam 登录失败: ${err.message}`)));
    // steam-user 没有 steamGuard 监听器时会从 stdin 读验证码，
    // 而 sidecar 的 stdin 是 JSON-RPC 通道，绝不能落到默认行为。
    const onSteamGuard = (
      domain: string | null,
      callback: (code: string) => void,
      lastCodeWrong: boolean,
    ) => {
      if (domain) {
        finish(() =>
          reject(
            new Error(
              `需要邮箱 Steam Guard 验证码（发送至 ${domain}），无法后台登录；请为 bot 账号启用手机令牌并配置 shared secret`,
            ),
          ),
        );
        return;
      }
      if (!secret) {
        finish(() =>
          reject(new Error('账号启用了 Steam Guard，请在 Valve bot 设置中填写 shared secret')),
        );
        return;
      }
      if (lastCodeWrong) {
        finish(() =>
          reject(new Error('Steam Guard 验证码被拒绝，请检查 shared secret 是否正确')),
        );
        return;
      }
      callback(SteamTotp.getAuthCode(secret));
    };
    steamUser.on('loggedOn', onLoggedOn);
    steamUser.on('error', onError);
    (steamUser as EventEmitter).on('steamGuard', onSteamGuard);
    steamUser.logOn(details);
  });
  return promiseTimeout(attempt, { milliseconds: 45000, message: 'Steam 登录超时' });
}

async function attemptLogin(
  user: BotLogin,
  details: LogOnDetails,
): Promise<SteamClientLoginResult> {
  const steamUser = new SteamUser();
  let newRefreshToken: string | undefined;
  steamUser.on('refreshToken', (token: string) => {
    newRefreshToken = token;
  });

  try {
    await logOnOnce(steamUser, details, user.secret);
  } catch (err) {
    try {
      steamUser.logOff();
    } catch {
      // ignore
    }
    throw err;
  }

  // 密码登录时等待新的 refresh token 以便持久化；拿不到不算失败
  if (!('refreshToken' in details) && !newRefreshToken) {
    await promiseTimeout(
      new Promise<void>((resolve) => {
        steamUser.once('refreshToken', () => resolve());
      }),
      { milliseconds: 15000 },
    ).catch(() => undefined);
  }

  // 登录之后的 error 事件（如掉线）没有监听器会直接崩掉 sidecar 进程
  steamUser.on('error', (err: Error) => {
    console.error(`[sidecar] steam client error: ${err.message}`);
  });

  return { client: steamUser, refreshToken: newRefreshToken };
}

export async function loginSteamClient(user: BotLogin): Promise<SteamClientLoginResult> {
  if (user.refreshToken) {
    try {
      return await attemptLogin(user, { refreshToken: user.refreshToken });
    } catch (err) {
      if (!user.password) throw err;
      console.error(
        `[sidecar] steam refresh token login failed, falling back to password: ${err instanceof Error ? err.message : err}`,
      );
    }
  }

  if (!user.password) {
    throw new Error('Valve bot password is required when no refresh token is stored');
  }

  return attemptLogin(user, {
    accountName: user.username,
    password: user.password,
    twoFactorCode: user.secret ? SteamTotp.getAuthCode(user.secret) : undefined,
  });
}
