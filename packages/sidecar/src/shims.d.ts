declare module 'unbzip2-stream';
declare module 'globaloffensive' {
  import type { EventEmitter } from 'node:events';
  export interface Match {
    matchid: string;
    demo_url?: string;
  }
  export default class GlobalOffensive extends EventEmitter {
    constructor(steamUser: unknown);
    requestGame(matchId: string): void;
    on(event: 'matchList', listener: (matches: Match[]) => void): this;
  }
}
declare module 'steam-user';
