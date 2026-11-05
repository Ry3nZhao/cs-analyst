// W3-C：全局的拖拽导入、通知点击跳转、监听目录自动导入的提示。
// 视觉部分在 components/OnboardingGlobal.vue（app.vue 里挂一次）。
import { listen } from '@tauri-apps/api/event';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import type { ReplayImportedDto } from '@cs-analyst/shared-types';
import { tauriInvoke } from '~/composables/useTauri';
import { useDropState } from '~/composables/useDropState';

const isDemo = (p: string) => p.toLowerCase().endsWith('.dem');

export default defineNuxtPlugin(() => {
  const drop = useDropState();
  const toast = useToast();
  const router = useRouter();

  // 后台已有任务时 import_demos 会拒绝：稍后重试（最多约 1 分钟）
  async function importWithRetry(paths: string[], attempt = 0): Promise<void> {
    try {
      await tauriInvoke('import_demos', { filePaths: paths });
      toast.add({ title: `开始导入 ${paths.length} 个 demo`, description: '导入后会自动解析', color: 'neutral' });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (message.includes('后台任务') && attempt < 30) {
        if (attempt === 0) toast.add({ title: '有后台任务正在运行', description: '完成后自动导入拖入的文件', color: 'neutral' });
        setTimeout(() => void importWithRetry(paths, attempt + 1), 2000);
        return;
      }
      toast.add({ title: '导入失败', description: message, color: 'error' });
    }
  }

  getCurrentWebview()
    .onDragDropEvent((event) => {
      const p = event.payload;
      if (p.type === 'enter') {
        drop.value = {
          active: true,
          demoCount: p.paths.filter(isDemo).length,
          otherCount: p.paths.filter((x) => !isDemo(x)).length,
        };
      } else if (p.type === 'leave') {
        drop.value = { active: false, demoCount: 0, otherCount: 0 };
      } else if (p.type === 'drop') {
        drop.value = { active: false, demoCount: 0, otherCount: 0 };
        const demos = p.paths.filter(isDemo);
        if (demos.length === 0) {
          toast.add({ title: '没有可导入的文件', description: '只支持 CS2 的 .dem 文件', color: 'warning' });
          return;
        }
        void importWithRetry(demos);
      }
    })
    .catch(() => {
      /* 浏览器里直接打开页面 */
    });

  // 通知点击 / 托盘：后端要求跳到某个路由
  listen<string>('app_navigate', (e) => {
    if (typeof e.payload === 'string' && e.payload.startsWith('/')) void router.push(e.payload);
  }).catch(() => {});

  // 监听 CS2 replays 目录自动导入的结果
  listen<ReplayImportedDto>('replay_imported', (e) => {
    const name = e.payload.path.split(/[\\/]/).pop() ?? e.payload.path;
    if (e.payload.action === 'imported') {
      toast.add({ title: '已自动导入 CS2 录像', description: name, color: 'success' });
    } else if (e.payload.action === 'error') {
      toast.add({ title: '自动导入失败', description: `${name}：${e.payload.error ?? ''}`, color: 'error' });
    }
  }).catch(() => {});
});
