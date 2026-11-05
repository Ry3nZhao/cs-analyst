// 身份与对位（W1-C）。对应 Rust：src/tauri/src-tauri/src/identity/{query,manage,commands}.rs
//
// - 一个 person 下挂多个 identity；绑在一起的就是同一个人。
// - steamid 一律是十进制字符串（超出 JS 安全整数范围）。
// - 对局数据只通过 platform = 'steam' 的身份关联；平台账号（5eplay / renown ...）的 steamid 只用于自动关联。
// - 所有对位数字都排除了自杀和队友伤害。

/** 'steam' = demo 里的 Steam64；其余为平台账号 */
export type IdentityPlatform = 'steam' | '5eplay' | 'renown' | 'perfect' | (string & {});

/** 胜负：1 胜 / 0 平（或无法判断）/ -1 负 */
export type MatchResult = 1 | 0 | -1;

export interface PersonSummaryDto {
  id: number;
  /** 手动名 > 最近一场的游戏名 > 平台账号 id */
  displayName: string;
  /** 手动设置的显示名 */
  customName: string | null;
  note: string | null;
  tags: string[];
  /** 「我关注的人」（由 users / sub_accounts 迁移而来，或手动关注） */
  followed: boolean;
  userId: string | null;
  /** 已被合并时指向合并后的人 */
  mergedInto: number | null;
  steamids: string[];
  /** 本地库里出现的场数 */
  matchCount: number;
  lastSeenAt: string | null;
}

export interface IdentityDto {
  id: number;
  personId: number;
  platform: IdentityPlatform;
  externalId: string;
  steamid: string | null;
  subAccountId: string | null;
  lastName: string | null;
  matchCount: number;
  firstSeenAt: string | null;
  lastSeenAt: string | null;
  /** 手动拆分 / 撤销过：自动关联不会再移动它 */
  manual: boolean;
}

/** 历史游戏名 */
export interface AliasDto {
  name: string;
  steamid: string;
  matches: number;
  lastSeenAt: string | null;
}

export interface PersonMergeDto {
  id: number;
  targetId: number;
  sourceId: number;
  identityIds: number[];
  /** 自动关联产生的合并 */
  auto: boolean;
  createdAt: string;
}

export interface RecordDto {
  matches: number;
  wins: number;
  losses: number;
  ties: number;
  /** wins / matches；没有场次时为 null */
  winRate: number | null;
}

export interface PersonDetailDto {
  person: PersonSummaryDto;
  identities: IdentityDto[];
  aliases: AliasDto[];
  /** 并进这个人、还能撤销的合并（新的在前） */
  merges: PersonMergeDto[];
  record: RecordDto;
}

export interface DuelRecordDto {
  total: number;
  won: number;
  lost: number;
  /** won / (won + lost)；没有分出胜负时为 null */
  winRate: number | null;
}

export interface H2hMatchDto {
  matchId: string;
  playedAt: string | null;
  mapName: string | null;
  platform: string;
  sameTeam: boolean;
  /** a 的胜负 */
  result: MatchResult;
  /** a 击杀 b */
  kills: number;
  /** b 击杀 a */
  deaths: number;
}

/** 交手数据，所有数字都是 a 的视角 */
export interface HeadToHeadDto {
  a: PersonSummaryDto;
  b: PersonSummaryDto;
  /** 同队 */
  together: RecordDto;
  /** 对手（wins = a 赢） */
  against: RecordDto;
  kills: number;
  deaths: number;
  /** 实际扣血（按剩余血量封顶） */
  damage: number;
  damageTaken: number;
  /** 每回合两人第一次交火（间隔 ≤ 3 秒连成一串，串内先杀对方者胜，否则平） */
  duels: DuelRecordDto;
  /** 回合首杀发生在两人之间 */
  openings: DuelRecordDto;
  /** 最近 50 场同场对局 */
  matches: H2hMatchDto[];
}

export interface TeammateDto {
  person: PersonSummaryDto;
  /** 同队时的战绩 */
  record: RecordDto;
}

export interface TeammatesDto {
  person: PersonSummaryDto;
  /** 总战绩，用于和组排胜率对比 */
  overall: RecordDto;
  teammates: TeammateDto[];
}

/** update_person 的 patch：字段缺省 = 不改；displayName / note 传空字符串 = 清空 */
export interface PersonPatchDto {
  displayName?: string;
  note?: string;
  tags?: string[];
  followed?: boolean;
}

/**
 * Tauri 命令（invoke 参数用 camelCase）：
 * - search_persons({ query?, followedOnly?, limit? = 50 }) → PersonSummaryDto[]
 * - get_person({ id }) → PersonDetailDto（传入已被合并的 id 时返回合并后的人）
 * - get_head_to_head({ a, b }) → HeadToHeadDto
 * - list_teammates({ person, minMatches? = 2, limit? = 50 }) → TeammatesDto
 * - merge_persons({ target, source }) → PersonDetailDto（target）
 * - undo_merge({ mergeId }) → PersonDetailDto（恢复出来的 source）
 * - split_identity({ identityId, toPerson? }) → PersonDetailDto（身份现在所属的人）
 * - update_person({ id, patch: PersonPatchDto }) → PersonDetailDto
 * - rebuild_head_to_head() → number（处理的对局数）
 */
export type IdentityCommand =
  | 'search_persons'
  | 'get_person'
  | 'get_head_to_head'
  | 'list_teammates'
  | 'merge_persons'
  | 'undo_merge'
  | 'split_identity'
  | 'update_person'
  | 'rebuild_head_to_head';
