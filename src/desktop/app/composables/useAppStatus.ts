import type { BackgroundTaskDto } from '@cs-analyst/shared-types';
import { listenBackgroundTask, listenBackgroundTaskLog, tauriInvoke } from './useTauri';

export function useAppStatus() {
  const status = useState('app-status', () => ({
    sidecarRunning: false,
    orchestratorRunning: false,
    queuePaused: false,
    dataRoot: '',
    lastDiscoverMinutesAgo: null as number | null,
    backgroundTask: null as BackgroundTaskDto | null,
  }));
  const logs = useState<string[]>('background-task-logs', () => []);
  const logOpen = useState('log-drawer-open', () => false);

  async function refreshStatus() {
    try {
      const s = await tauriInvoke<typeof status.value>('get_app_status');
      status.value = s;
    } catch {
      /* dev without tauri */
    }
  }

  async function discoverNow() {
    await tauriInvoke('discover_now');
    await refreshStatus();
  }

  async function toggleQueue() {
    await tauriInvoke('toggle_queue_pause');
    await refreshStatus();
  }

  async function refreshTaskLogs() {
    try {
      logs.value = await tauriInvoke<string[]>('get_background_task_logs');
    } catch {
      /* dev without tauri */
    }
  }

  // Reload snapshot whenever the drawer is opened
  watch(logOpen, (open: boolean) => {
    if (open) void refreshTaskLogs();
  });

  function listenBackgroundWork() {
    const u1 = listenBackgroundTask((task) => {
      status.value.backgroundTask = task;
      void refreshStatus();
    });
    const u2 = listenBackgroundTaskLog((message) => {
      logs.value = [...logs.value.slice(-399), message];
    });
    void refreshTaskLogs();
    return () => {
      u1();
      u2();
    };
  }

  return { status, logs, logOpen, refreshStatus, refreshTaskLogs, listenBackgroundWork, discoverNow, toggleQueue };
}
