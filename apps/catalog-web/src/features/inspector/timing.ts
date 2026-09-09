export type TimingPhase = {
  name:
    | 'candidate'
    | 'page'
    | 'related'
    | 'serialize'
    | 'frame-load'
    | 'frame-fallback';
  duration: number;
};

export type TimingEntry = {
  phases: TimingPhase[];
  recordedAt: number;
};

const maximumEntries = 50;
const serverPhases = new Set(['candidate', 'page', 'related', 'serialize']);
const framePhases = new Set(['frame-load', 'frame-fallback']);
const entries: TimingEntry[] = [];
const listeners = new Set<() => void>();

const notify = () => listeners.forEach((listener) => listener());

const addEntry = (phases: TimingPhase[]) => {
  if (!__CATALOG_DEVTOOLS__ || phases.length === 0) return;
  entries.unshift({ phases, recordedAt: Date.now() });
  entries.splice(maximumEntries);
  notify();
};

/** Accept only the fixed, aggregate names emitted by the API. */
export const recordServerTiming = (header: string | null) => {
  if (!__CATALOG_DEVTOOLS__ || !header) return;
  const phases = header.split(',').flatMap((part) => {
    const match = /^\s*([a-z-]+);dur=([0-9]+(?:\.[0-9]+)?)\s*$/.exec(part);
    if (!match || !serverPhases.has(match[1])) return [];
    const duration = Number(match[2]);
    return Number.isFinite(duration) && duration >= 0
      ? [{ name: match[1] as TimingPhase['name'], duration }]
      : [];
  });
  addEntry(phases);
};

/** Frame timings intentionally carry no contribution, entity, or artifact data. */
export const recordFrameTiming = (
  name: 'frame-load' | 'frame-fallback',
  duration: number,
) => {
  if (!framePhases.has(name) || !Number.isFinite(duration) || duration < 0)
    return;
  addEntry([{ name, duration }]);
};

export const recentTimings = () => [...entries];
export const subscribeTimings = (listener: () => void) => {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
};

export const clearTimingsForTest = () => {
  entries.length = 0;
};
