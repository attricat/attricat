export type ApiHealthState = 'checking' | 'ready' | 'restarting';

export const apiHealthStatusKeys = {
  checking: 'inspector.checking',
  ready: 'inspector.ready',
  restarting: 'inspector.restarting',
} as const satisfies Record<ApiHealthState, string>;

export const apiHealthSummaryKeys = {
  checking: 'inspector.checkingApi',
  ready: 'inspector.apiReady',
  restarting: 'inspector.apiRestarting',
} as const satisfies Record<ApiHealthState, string>;

export const apiHealthColor = (state: ApiHealthState) =>
  state === 'ready' ? 'success.main' : 'warning.main';
