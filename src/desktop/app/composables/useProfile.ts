import type { PercentileDto, ProfileDto, ProfileMetricKey, ProfileOptionsDto, SampleScope } from '@cs-analyst/shared-types';
import { tauriInvoke } from './useTauri';

/** 个人主页数据（W3-B）。 */
export async function fetchProfile(steamid: string, options: ProfileOptionsDto): Promise<ProfileDto> {
  return tauriInvoke<ProfileDto>('get_profile', { steamid, options });
}

/** 失误集锦（W3-A 的页面）。 */
export const MISTAKES_LINK = '/highlights?type=mistake&days=30';

const DIGITS: Record<ProfileMetricKey, number> = {
  adr: 1,
  kast: 1,
  kd: 2,
  rating: 2,
  swing: 2,
  openingDuel: 1,
  clutch: 0,
  ttd: 0,
  crosshairPlacement: 1,
  counterStrafe: 1,
  sprayError: 2,
};

/** 指标值的显示（不含单位）。null → 「不可用」由调用方处理。 */
export function fmtMetric(key: ProfileMetricKey, v: number | null | undefined): string {
  if (v == null || !Number.isFinite(v)) return '—';
  const s = v.toFixed(DIGITS[key] ?? 1);
  return key === 'swing' && v > 0 ? `+${s}` : s;
}

export function fmtRate(v: number | null | undefined, digits = 0): string {
  return v == null ? '—' : `${v.toFixed(digits)}%`;
}

/** 百分位的语义色：≥ 67 好，≤ 33 差，中间中性。 */
export function percentileTone(p: number | null | undefined): 'success' | 'danger' | 'neutral' {
  if (p == null) return 'neutral';
  if (p >= 67) return 'success';
  if (p <= 33) return 'danger';
  return 'neutral';
}

export function scopeLabel(scope: SampleScope): string {
  return { tier: '同段位', all: '全库', none: '样本不足' }[scope];
}

/** 「第 62 百分位 · 全库 27 人」之类的说明。 */
export function percentileText(p: PercentileDto): string {
  if (p.scope === 'none') return `样本不足（${p.sampleSize} 人）`;
  if (p.value == null) return '不可用';
  return `超过 ${p.value.toFixed(0)}% · ${scopeLabel(p.scope)} ${p.sampleSize} 人`;
}

export function shortDate(at: string | null | undefined): string {
  if (!at) return '';
  const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(at);
  return m ? `${m[2]}-${m[3]}` : at;
}

export function mapShort(map: string | null | undefined): string {
  if (!map) return '未知地图';
  return map.replace(/^(de|cs|ar)_/, '').replace(/^\w/, (c) => c.toUpperCase());
}

export function resultLabel(r: 'win' | 'loss' | 'draw'): string {
  return { win: '胜', loss: '负', draw: '平' }[r];
}
