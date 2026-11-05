/** 六个训练模式的元数据。`score` 列的口径（名称、单位、方向）以这里为准。 */
import type { TrainingModeMeta } from '@cs-analyst/shared-types';

export const TRAINING_MODES: TrainingModeMeta[] = [
  {
    mode: 'reaction',
    label: '反应测试',
    description: '屏幕变色后尽快点击，提前点击判为犯规。',
    scoreLabel: '中位数',
    scoreUnit: 'ms',
    higherIsBetter: false,
    pointerLock: false,
  },
  {
    mode: 'flick',
    label: '甩枪',
    description: '目标随机出现在视野里，在限时内甩过去点掉。',
    scoreLabel: '命中率',
    scoreUnit: '%',
    higherIsBetter: true,
    pointerLock: true,
  },
  {
    mode: 'tracking',
    label: '跟枪',
    description: '目标会变向平移，尽量让准星一直留在它身上。',
    scoreLabel: '在目标上',
    scoreUnit: '%',
    higherIsBetter: true,
    pointerLock: true,
  },
  {
    mode: 'micro',
    label: '微调',
    description: '小目标出现在准星附近几度内，练精细修正。',
    scoreLabel: '精度',
    scoreUnit: '%',
    higherIsBetter: true,
    pointerLock: true,
  },
  {
    mode: 'strafe',
    label: '急停节奏',
    description: '按住 A / D 移动，反向急停，在速度降到精准阈值内时开枪。',
    scoreLabel: '急停成功率',
    scoreUnit: '%',
    higherIsBetter: true,
    pointerLock: false,
  },
  {
    mode: 'spray',
    label: '压枪练习',
    description: '按住左键连发，把准星沿弹道反方向拉回来，让子弹留在第一发的位置。',
    scoreLabel: '控枪',
    scoreUnit: '%',
    higherIsBetter: true,
    pointerLock: true,
  },
];
