import type { TrainingMode } from '@cs-analyst/shared-types';
import type { TrainingModeImpl } from '../core';
import { ReactionMode } from './reaction';
import { FLICK_SIZES, MICRO_SIZES, TargetsMode } from './targets';
import { TRACKING_SIZES, TRACKING_SPEEDS, TrackingMode } from './tracking';
import { StrafeMode, type StrafeWeapon } from './strafe';
import { SprayMode } from './spray';

export type TargetSize = 'L' | 'M' | 'S';

/** 设置页上可调的选项（所有模式共用一个对象，各取所需） */
export interface TrainingOptions {
  count: number;
  durationS: number;
  size: TargetSize;
  timeLimitMs: number;
  speed: keyof typeof TRACKING_SPEEDS;
  strafeWeapon: StrafeWeapon;
  sprayWeapon: string;
  sprays: number;
}

export const DEFAULT_OPTIONS: Record<TrainingMode, TrainingOptions> = {
  reaction: { count: 5, durationS: 30, size: 'M', timeLimitMs: 1500, speed: 'medium', strafeWeapon: 'ak47', sprayWeapon: 'ak47', sprays: 8 },
  flick: { count: 30, durationS: 30, size: 'M', timeLimitMs: 1500, speed: 'medium', strafeWeapon: 'ak47', sprayWeapon: 'ak47', sprays: 8 },
  tracking: { count: 30, durationS: 30, size: 'M', timeLimitMs: 1500, speed: 'medium', strafeWeapon: 'ak47', sprayWeapon: 'ak47', sprays: 8 },
  micro: { count: 40, durationS: 30, size: 'M', timeLimitMs: 2500, speed: 'medium', strafeWeapon: 'ak47', sprayWeapon: 'ak47', sprays: 8 },
  strafe: { count: 20, durationS: 30, size: 'M', timeLimitMs: 1500, speed: 'medium', strafeWeapon: 'ak47', sprayWeapon: 'ak47', sprays: 8 },
  spray: { count: 30, durationS: 30, size: 'M', timeLimitMs: 1500, speed: 'medium', strafeWeapon: 'ak47', sprayWeapon: 'ak47', sprays: 8 },
};

export function createMode(mode: TrainingMode, o: TrainingOptions): TrainingModeImpl {
  switch (mode) {
    case 'reaction':
      return new ReactionMode({ count: o.count });
    case 'flick':
      return new TargetsMode({ kind: 'flick', count: o.count, radiusDeg: FLICK_SIZES[o.size], timeLimitMs: o.timeLimitMs, minDist: 10, maxDist: 38 });
    case 'micro':
      return new TargetsMode({ kind: 'micro', count: o.count, radiusDeg: MICRO_SIZES[o.size], timeLimitMs: o.timeLimitMs, minDist: 1.2, maxDist: 4.5 });
    case 'tracking':
      return new TrackingMode({ durationS: o.durationS, radiusDeg: TRACKING_SIZES[o.size], maxSpeed: TRACKING_SPEEDS[o.speed] });
    case 'strafe':
      return new StrafeMode({ count: o.count, weapon: o.strafeWeapon });
    case 'spray':
      return new SprayMode({ weapon: o.sprayWeapon, sprays: o.sprays });
  }
}

/** 记进 settings 的模式参数（只留和该模式相关的） */
export function optionsSnapshot(mode: TrainingMode, o: TrainingOptions): Record<string, unknown> {
  switch (mode) {
    case 'reaction':
      return { count: o.count };
    case 'flick':
      return { count: o.count, size: o.size, radiusDeg: FLICK_SIZES[o.size], timeLimitMs: o.timeLimitMs };
    case 'micro':
      return { count: o.count, size: o.size, radiusDeg: MICRO_SIZES[o.size], timeLimitMs: o.timeLimitMs };
    case 'tracking':
      return { durationS: o.durationS, size: o.size, radiusDeg: TRACKING_SIZES[o.size], speed: o.speed, maxSpeed: TRACKING_SPEEDS[o.speed] };
    case 'strafe':
      return { count: o.count, weapon: o.strafeWeapon };
    case 'spray':
      return { weapon: o.sprayWeapon, sprays: o.sprays };
  }
}
