/**
 * Host limits on one extension's native UI. They are host decisions, not
 * manifest fields: exceeding any fails the extension's Worker with a code.
 */
export const nativeUiBudgets = {
  /** Remote nodes across all of an extension's mounted contributions. */
  maximumTreeNodes: 5_000,
  /** Mutation records an extension may send within one rate window. */
  maximumMutationsPerWindow: 5_000,
  mutationWindow: 1_000,
  /** The largest message an extension's Worker may send. */
  maximumMessageBytes: 1_048_576,
  /** How long a Worker has to register its components. */
  startTimeout: 10_000,
  /** How often the host asks a Worker to answer, and how long it waits. */
  heartbeatInterval: 5_000,
  heartbeatTimeout: 4_000,
  /** Input changes are sent once typing pauses this long. */
  inputDebounce: 250,
} as const;

/** Why an extension's native UI stopped; each has a translated message. */
export const nativeUiErrorCodes = {
  startFailed: 'native_ui_start_failed',
  startTimedOut: 'native_ui_start_timed_out',
  notRegistered: 'native_ui_not_registered',
  unresponsive: 'native_ui_unresponsive',
  treeTooLarge: 'native_ui_tree_too_large',
  mutationRateExceeded: 'native_ui_mutation_rate_exceeded',
  messageTooLarge: 'native_ui_message_too_large',
  renderFailed: 'native_ui_render_failed',
} as const;
export type NativeUiErrorCode =
  (typeof nativeUiErrorCodes)[keyof typeof nativeUiErrorCodes];

/** Messages between the host, the shared sandbox frame and a Worker. */
export const nativeMessageTypes = {
  /** Host to frame, once: the control port. */
  sandboxInit: 'attricat:native-sandbox-init.v1',
  /** Host to frame: start a Worker from an artifact with its own port. */
  start: 'attricat:native-start.v1',
  /** Host to frame: terminate a Worker. */
  stop: 'attricat:native-stop.v1',
  /** Frame to host: a Worker failed to load or threw. */
  workerError: 'attricat:native-worker-error.v1',
  /** Frame to Worker: the extension's port, which the SDK takes over. */
  workerInit: 'attricat:native-init.v1',
} as const;

/** How a native contribution's mount is going. */
export const nativeMountStates = {
  starting: 'starting',
  ready: 'ready',
  failed: 'failed',
} as const;
export type NativeMountState =
  (typeof nativeMountStates)[keyof typeof nativeMountStates];
