import { create } from 'zustand';

export type TimingPhase = {
  name: string;
  duration: number;
  queryCount?: number;
};

export type TimingEntry = {
  phases: TimingPhase[];
  recordedAt: number;
};

const maximumEntries = 50;
const serverPhases = new Set([
  'candidate',
  'page',
  'related',
  'serialize',
  'sql',
  'sql-blueprint-load',
  'sql-table-sort-resolve',
  'sql-search-resolve',
  'sql-relationship-facet',
  'sql-entities-page',
  'sql-related-hydrate',
]);
const framePhases = new Set(['frame-load', 'frame-fallback']);
type TimingStore = {
  entries: TimingEntry[];
  add: (phases: TimingPhase[]) => void;
  clear: () => void;
};

export const useTimingStore = create<TimingStore>((set) => ({
  entries: [],
  add: (phases) =>
    set((state) => ({
      entries: [{ phases, recordedAt: Date.now() }, ...state.entries].slice(
        0,
        maximumEntries,
      ),
    })),
  clear: () => set({ entries: [] }),
}));

const addEntry = (phases: TimingPhase[]) => {
  if (!__CATALOG_DEVTOOLS__ || phases.length === 0) return;
  useTimingStore.getState().add(phases);
};

/** Accept only the fixed, aggregate names emitted by the API. */
export const recordServerTiming = (header: string | null) => {
  if (!__CATALOG_DEVTOOLS__ || !header) return;
  const phases = header.split(',').flatMap((part) => {
    const match =
      /^\s*([a-z-]+);dur=([0-9]+(?:\.[0-9]+)?)(?:;desc=queries-([0-9]+))?\s*$/.exec(
        part,
      );
    if (!match || !serverPhases.has(match[1])) return [];
    const duration = Number(match[2]);
    const queryCount =
      match[1].startsWith('sql') && match[3] ? Number(match[3]) : undefined;
    if (
      !Number.isFinite(duration) ||
      duration < 0 ||
      (queryCount !== undefined && !Number.isSafeInteger(queryCount))
    )
      return [];
    return [
      {
        name: match[1],
        duration,
        ...(queryCount === undefined ? {} : { queryCount }),
      },
    ];
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

export const recentTimings = () => [...useTimingStore.getState().entries];

export const clearTimingsForTest = () => useTimingStore.getState().clear();
