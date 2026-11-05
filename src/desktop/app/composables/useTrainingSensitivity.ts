/**
 * 训练用的鼠标灵敏度：首次自动读取本机 CS2 配置，之后记在本地（localStorage）。
 * 读取逻辑在 Rust：`detect_cs2_sensitivity` / `read_cs2_sensitivity`（src-tauri/src/training/）。
 */
import type { Cs2SensitivityCandidate, Cs2SensitivityDetect } from '@cs-analyst/shared-types';
import { tauriInvoke } from '~/composables/useTauri';

export interface TrainingSensitivity {
  value: number;
  source: 'cs2' | 'file' | 'manual';
  path?: string | null;
  steamid?: string | null;
}

const KEY = 'training.sensitivity';

function load(): TrainingSensitivity | null {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return null;
    const v = JSON.parse(raw) as TrainingSensitivity;
    return Number.isFinite(v.value) && v.value > 0 ? v : null;
  } catch {
    return null;
  }
}

function save(v: TrainingSensitivity | null) {
  try {
    if (v) localStorage.setItem(KEY, JSON.stringify(v));
    else localStorage.removeItem(KEY);
  } catch {
    /* ignore */
  }
}

export function useTrainingSensitivity() {
  const current = useState<TrainingSensitivity | null>('training-sensitivity', () => load());
  const candidates = useState<Cs2SensitivityCandidate[]>('training-sens-candidates', () => []);
  const detecting = useState('training-sens-detecting', () => false);
  const detectError = useState<string | null>('training-sens-error', () => null);
  const detected = useState('training-sens-detected', () => false);

  function set(v: TrainingSensitivity) {
    current.value = v;
    save(v);
  }

  async function detect(apply = true) {
    detecting.value = true;
    detectError.value = null;
    try {
      const r = await tauriInvoke<Cs2SensitivityDetect>('detect_cs2_sensitivity');
      candidates.value = r.candidates;
      const best = r.candidates.find((c) => c.sensitivity != null);
      if (!best) {
        detectError.value = r.steamPath
          ? `在 ${r.steamPath} 下没有找到 CS2 的配置文件`
          : '没有找到 Steam 安装目录';
      } else if (apply) {
        set({ value: best.sensitivity!, source: 'cs2', path: best.path, steamid: best.steamid });
      }
    } catch (e) {
      detectError.value = String(e);
    } finally {
      detecting.value = false;
      detected.value = true;
    }
  }

  async function pickFile() {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const path = await open({
      multiple: false,
      directory: false,
      title: '选择 CS2 配置文件',
      filters: [{ name: 'CS2 配置', extensions: ['vcfg', 'cfg'] }],
    });
    if (!path || Array.isArray(path)) return;
    try {
      const value = await tauriInvoke<number>('read_cs2_sensitivity', { path });
      set({ value, source: 'file', path });
      detectError.value = null;
    } catch (e) {
      detectError.value = `${path}：${e}`;
    }
  }

  /** 首次进入时自动读取一次 */
  async function ensure() {
    if (current.value || detected.value || detecting.value) return;
    await detect(true);
  }

  return { current, candidates, detecting, detectError, set, detect, pickFile, ensure };
}
