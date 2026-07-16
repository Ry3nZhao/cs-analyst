/** Adapted from cs-demo-downloader (MIT) — Auth Code batch resolve types */

export interface BotLogin {
  username: string;
  password?: string;
  secret?: string;
  refreshToken?: string;
}

export interface ResolveItem {
  jobId: string;
  shareCode: string;
  matchId: string;
}

export interface ResolvedValveMatch {
  jobId: string;
  shareCode: string;
  matchId: string;
  demoUrl?: string;
  playedAt?: string;
  mapName?: string;
  error?: string;
}

export interface ResolveValveAuthcodeResult {
  resolved: ResolvedValveMatch[];
  refreshToken?: string;
}
