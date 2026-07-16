declare module 'unbzip2-stream';

declare module 'globaloffensive' {
  import type { EventEmitter } from 'node:events';

  export interface GcMatch {
    matchid: string;
    demo_url?: string;
    matchtime?: number;
    roundstatsall?: Array<{
      map?: string;
      b_switched_teams?: boolean;
      reservation?: { account_ids: number[] };
    }>;
  }

  export default class GlobalOffensive extends EventEmitter {
    constructor(steamUser: unknown);
    requestGame(shareCode: string): void;
    on(event: 'matchList', listener: (matches: GcMatch[]) => void): this;
    on(event: 'connectedToGC', listener: () => void): this;
  }
}

declare module 'steam-user' {
  import { EventEmitter } from 'node:events';

  export default class SteamUser extends EventEmitter {
    logOn(details: Record<string, unknown>): void;
    logOff(): void;
    gamesPlayed(games: number | number[] | [], force?: boolean): void;
    on(event: 'loggedOn', listener: () => void): this;
    on(event: 'appLaunched', listener: (id: number) => void): this;
    on(event: 'appQuit', listener: (id: number) => void): this;
    on(event: 'refreshToken', listener: (token: string) => void): this;
    on(event: 'error', listener: (err: Error) => void): this;
  }
}

declare module 'steam-totp' {
  export function getAuthCode(secret: string): string;
}
