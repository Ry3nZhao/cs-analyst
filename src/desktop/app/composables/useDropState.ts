// W3-C：拖拽导入的悬停状态（plugins/onboarding.client.ts 写，OnboardingGlobal.vue 读）
export interface DropState {
  /** 正在拖着文件悬停在窗口上 */
  active: boolean;
  /** 其中 .dem 文件数量（enter 时就知道） */
  demoCount: number;
  otherCount: number;
}

export function useDropState() {
  return useState<DropState>('onboarding-drop', () => ({ active: false, demoCount: 0, otherCount: 0 }));
}
