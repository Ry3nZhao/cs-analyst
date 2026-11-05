// W3-C：首次启动（没走过向导，且没有主玩家、账号、对局）时自动进入 /welcome。只在应用启动后的第一次导航检查。
import type { OnboardingStateDto } from '@cs-analyst/shared-types';
import { tauriInvoke } from '~/composables/useTauri';

let checked = false;

export default defineNuxtRouteMiddleware(async (to) => {
  if (checked) return;
  checked = true;
  if (to.path === '/welcome') return;
  try {
    const state = await tauriInvoke<OnboardingStateDto>('get_onboarding_state');
    if (state.needsOnboarding) return navigateTo('/welcome');
  } catch {
    /* 浏览器里直接打开页面 */
  }
});
