import { tauriInvoke } from './useTauri';

export function useAppStatus() {
  const status = useState('app-status', () => ({
    sidecarRunning: false,
    orchestratorRunning: false,
    queuePaused: false,
    dataRoot: '',
    lastDiscoverMinutesAgo: null as number | null,
  }));

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

  return { status, refreshStatus, discoverNow, toggleQueue };
}
