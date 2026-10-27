export const apiHealthStates = {
  checking: 'checking',
  ready: 'ready',
  restarting: 'restarting',
} as const;
export type ApiHealthState =
  (typeof apiHealthStates)[keyof typeof apiHealthStates];

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
  state === apiHealthStates.ready ? 'success.main' : 'warning.main';
