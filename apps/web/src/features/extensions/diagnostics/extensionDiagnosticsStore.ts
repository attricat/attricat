import { create } from 'zustand';
import { errorMessage } from '../../../api/errors';
import { inspectorEnabled } from '../../inspector/enabled';
import type { ExtensionContribution } from '../api';
import { renderingModes } from '../constants';
import {
  handleBrokerRequest,
  type BrokerDependencies,
} from '../extensionBroker';

/** How much the Inspector keeps of each history, newest first. */
export const extensionDiagnosticsLimits = {
  requests: 100,
  drops: 50,
  /** Validation errors can be long; the Inspector shows their start. */
  reasonLength: 300,
} as const;

export const extensionRunStates = {
  starting: 'starting',
  ready: 'ready',
  failed: 'failed',
} as const;
export type ExtensionRunState =
  (typeof extensionRunStates)[keyof typeof extensionRunStates];

export const brokerOutcomes = { allowed: 'allowed', denied: 'denied' } as const;
export type BrokerOutcome =
  (typeof brokerOutcomes)[keyof typeof brokerOutcomes];

/** One running extension UI: a native Worker or a frame contribution. */
export type ExtensionRunRecord = {
  key: string;
  extensionId: string;
  extensionName: string;
  /** The frame contribution's id; a native Worker serves all of them. */
  contributionId?: string;
  rendering: (typeof renderingModes)[keyof typeof renderingModes];
  uiKit?: string;
  devRevision?: string;
  state: ExtensionRunState;
  /** A translated failure code, such as `native_ui_unresponsive`. */
  errorCode?: string;
  startedAt: number;
  /** From start to ready. */
  startMs?: number;
  heartbeatMs?: number;
  /** Native budgets in use: elements, updates in the busiest window, largest message. */
  nodes?: number;
  peakMutations?: number;
  largestMessageBytes?: number;
};

/** Something the host kept out of an extension's drawing. */
export type DropRecord = { at: number; extensionName: string; message: string };

/** One broker request: never its payload or answer. */
export type BrokerRequestRecord = {
  at: number;
  extensionName: string;
  contributionId: string;
  method: string;
  durationMs: number;
  outcome: BrokerOutcome;
  /** Why it was denied; extension code never sees this. */
  reason?: string;
};

type ExtensionDiagnosticsState = {
  runs: Record<string, ExtensionRunRecord>;
  drops: DropRecord[];
  requests: BrokerRequestRecord[];
  clear: () => void;
};

export const useExtensionDiagnosticsStore = create<ExtensionDiagnosticsState>()(
  (set) => ({
    runs: {},
    drops: [],
    requests: [],
    clear: () => set({ drops: [], requests: [] }),
  }),
);

/**
 * Recording does nothing unless this browser shows the Inspector, so
 * extension UI pays for diagnostics only where someone can read them.
 */
export const recordRun = (
  key: string,
  patch: Partial<ExtensionRunRecord> &
    Pick<ExtensionRunRecord, 'extensionId' | 'extensionName' | 'rendering'>,
) => {
  if (!inspectorEnabled) return;
  useExtensionDiagnosticsStore.setState((state) => {
    const previous = state.runs[key];
    const defaults = {
      state: extensionRunStates.starting,
      startedAt: Date.now(),
    };
    const next: ExtensionRunRecord = {
      ...defaults,
      ...previous,
      ...patch,
      key,
    };
    if (
      previous &&
      Object.entries(next).every(
        ([field, value]) =>
          previous[field as keyof ExtensionRunRecord] === value,
      )
    )
      return state;
    return { runs: { ...state.runs, [key]: next } };
  });
};

export const recordDrop = (extensionName: string, message: string) => {
  if (!inspectorEnabled) return;
  useExtensionDiagnosticsStore.setState((state) => ({
    drops: [{ at: Date.now(), extensionName, message }, ...state.drops].slice(
      0,
      extensionDiagnosticsLimits.drops,
    ),
  }));
};

const recordRequest = (request: BrokerRequestRecord) =>
  useExtensionDiagnosticsStore.setState((state) => ({
    requests: [request, ...state.requests].slice(
      0,
      extensionDiagnosticsLimits.requests,
    ),
  }));

/**
 * Runs a broker request and, with the Inspector on, records its method,
 * duration and outcome. A denial's reason is recorded for the Inspector and
 * rethrown unchanged; callers still hide it from extension code.
 */
export const brokerRequest = async (
  request: { method: unknown; payload: unknown },
  dependencies: BrokerDependencies,
) => {
  if (!inspectorEnabled) return handleBrokerRequest(request, dependencies);
  const started = performance.now();
  const contribution: ExtensionContribution = dependencies.contribution;
  const record = (outcome: BrokerOutcome, reason?: string) =>
    recordRequest({
      at: Date.now(),
      extensionName: contribution.extension_name,
      contributionId: contribution.id,
      method: typeof request.method === 'string' ? request.method : '?',
      durationMs: performance.now() - started,
      outcome,
      reason,
    });
  try {
    const answer = await handleBrokerRequest(request, dependencies);
    record(brokerOutcomes.allowed);
    return answer;
  } catch (error) {
    record(
      brokerOutcomes.denied,
      (error instanceof Error ? errorMessage(error) : String(error)).slice(
        0,
        extensionDiagnosticsLimits.reasonLength,
      ),
    );
    throw error;
  }
};
