import { Thread, type ThreadMessageTarget } from '@quilted/threads';
import type { RemoteConnection } from '@remote-dom/core';
import { getExtensionArtifact, type ExtensionContribution } from '../api';
import {
  extensionBrokerMethods,
  extensionOutlets,
  extensionProtocolErrors,
  maximumExtensionResponseBytes,
  renderingModes,
} from '../constants';
import {
  handleBrokerRequest,
  type BrokerDependencies,
} from '../extensionBroker';
import { jsonByteLength } from '../utf8';
import {
  nativeMessageTypes,
  nativeMountStates,
  nativeUiBudgets,
  nativeUiErrorCodes,
  type NativeMountState,
  type NativeUiErrorCode,
} from './constants';
import { NativeReceiver } from './receiver';
import { nativeSandboxDocument } from './sandboxDocument';

/** What a contribution's code receives when it mounts and on every update. */
export type NativeMountInit = {
  context: Record<string, unknown>;
  configuration: unknown;
  capabilities: readonly string[];
  locale: string;
  theme: { color_mode: 'light' | 'dark' };
  ui_kit: string;
};

/** What the host offers an extension's Worker. */
type HostExports = {
  ready: (registered: unknown) => Promise<void>;
  request: (
    mountId: unknown,
    method: unknown,
    payload: unknown,
  ) => Promise<unknown>;
};
/** What the SDK in an extension's Worker offers the host. */
type WorkerExports = {
  mount: (
    mountId: string,
    contributionId: string,
    connection: RemoteConnection,
    init: NativeMountInit,
  ) => Promise<void>;
  update: (mountId: string, init: NativeMountInit) => Promise<void>;
  unmount: (mountId: string) => Promise<void>;
  ping: () => Promise<void>;
};

/** One mounted contribution, as the host tracks it. */
type Mount = {
  contribution: ExtensionContribution;
  receiver: NativeReceiver;
  init: NativeMountInit;
  dependencies: BrokerDependencies;
  /** An attribute editor's value proposals. */
  onPropose?: (value: unknown) => void;
  state: NativeMountState;
  error?: NativeUiErrorCode;
  sent: boolean;
};

export type NativeMountSnapshot = {
  state: NativeMountState;
  error?: NativeUiErrorCode;
};

const development = import.meta.env.DEV;

/** Answers the size of a message from a Worker, stopping past `limit`. */
export const messageBytes = (value: unknown, limit: number) => {
  let total = 0;
  const stack: unknown[] = [value];
  const seen = new Set<object>();
  while (stack.length && total <= limit) {
    const item = stack.pop();
    if (typeof item === 'string') total += item.length * 2;
    else if (typeof item === 'number' || typeof item === 'boolean') total += 8;
    else if (item && typeof item === 'object' && !seen.has(item)) {
      seen.add(item);
      if (Array.isArray(item)) stack.push(...item);
      else if (item instanceof Map)
        item.forEach((entry, key) => stack.push(key, entry));
      else if (item instanceof Set) item.forEach((entry) => stack.push(entry));
      else
        for (const [key, entry] of Object.entries(item)) {
          total += key.length * 2;
          stack.push(entry);
        }
    }
  }
  return total;
};

/**
 * Where extension Workers run. It is a host decision: today every Worker
 * starts in one shared sandbox frame, and an extension could later get its
 * own frame without any change to extensions or this host.
 */
export type NativeIsolation = {
  /** Starts a Worker from the artifact, giving it only `port`. */
  start: (
    key: string,
    artifact: string,
    port: MessagePort,
    onError: () => void,
  ) => Promise<void>;
  stop: (key: string) => void;
};

/**
 * The session's hidden sandbox frame. Every extension's Worker starts inside
 * it, never in Attricat's realm, and each Worker gets only its own port.
 */
class NativeSandbox implements NativeIsolation {
  #control: Promise<MessagePort> | undefined;
  #errors = new Map<string, () => void>();

  #frame() {
    this.#control ??= new Promise((resolve) => {
      const frame = document.createElement('iframe');
      frame.setAttribute('sandbox', 'allow-scripts');
      frame.srcdoc = nativeSandboxDocument;
      frame.hidden = true;
      frame.tabIndex = -1;
      frame.title = 'Attricat extensions';
      frame.setAttribute('aria-hidden', 'true');
      frame.addEventListener(
        'load',
        () => {
          const channel = new MessageChannel();
          channel.port1.onmessage = ({ data }) => {
            if (data?.type === nativeMessageTypes.workerError)
              this.#errors.get(String(data.key))?.();
          };
          frame.contentWindow?.postMessage(
            { type: nativeMessageTypes.sandboxInit },
            '*',
            [channel.port2],
          );
          resolve(channel.port1);
        },
        { once: true },
      );
      document.body.append(frame);
    });
    return this.#control;
  }

  async start(
    key: string,
    artifact: string,
    port: MessagePort,
    onError: () => void,
  ) {
    this.#errors.set(key, onError);
    (await this.#frame()).postMessage(
      { type: nativeMessageTypes.start, key, artifact },
      [port],
    );
  }

  stop(key: string) {
    this.#errors.delete(key);
    void this.#control?.then((control) =>
      control.postMessage({ type: nativeMessageTypes.stop, key }),
    );
  }
}

const sandbox = new NativeSandbox();

/** Where a Worker's identity comes from: the release and its build. */
export const nativeHostKey = (contribution: ExtensionContribution) =>
  `${contribution.extension_id}:${contribution.release_id}:${contribution.artifact_revision ?? ''}`;

/**
 * One extension's Worker and its mounted contributions. The port is the
 * extension's identity: every request it sends is attributed to this host
 * and to one of the mounts the host created on it, never to identifiers the
 * Worker supplies.
 */
export class NativeExtensionHost {
  readonly key: string;
  readonly extensionName: string;
  #declared: Set<string>;
  #registered: Set<string> | undefined;
  #mounts = new Map<string, Mount>();
  #nextMount = 0;
  #thread: Thread<WorkerExports, HostExports> | undefined;
  #abort = new AbortController();
  #timers: number[] = [];
  #listeners = new Set<() => void>();
  #error: NativeUiErrorCode | undefined;
  #nodes = 0;
  #windowStart = 0;
  #windowMutations = 0;
  #warned = new Set<string>();

  constructor(
    contributions: ExtensionContribution[],
    readonly isolation: NativeIsolation = sandbox,
  ) {
    const [first] = contributions;
    this.key = nativeHostKey(first);
    this.extensionName = first.extension_name;
    this.#declared = new Set(contributions.map((item) => item.id));
    void this.#start(first);
  }

  get error() {
    return this.#error;
  }

  /** Native contributions the runtime currently declares for this release. */
  declare(contributions: ExtensionContribution[]) {
    this.#declared = new Set(contributions.map((item) => item.id));
  }

  subscribe(listener: () => void) {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  snapshot(mountId: string): NativeMountSnapshot {
    const mount = this.#mounts.get(mountId);
    if (this.#error)
      return { state: nativeMountStates.failed, error: this.#error };
    return mount
      ? { state: mount.state, error: mount.error }
      : { state: nativeMountStates.starting };
  }

  /** Starts drawing a contribution once the Worker has registered it. */
  mount(
    contribution: ExtensionContribution,
    init: NativeMountInit,
    dependencies: BrokerDependencies,
    onPropose?: (value: unknown) => void,
  ) {
    const mountId = String(++this.#nextMount);
    const receiver = new NativeReceiver(init.ui_kit, {
      countNodes: (delta) => this.#countNodes(delta),
      countMutations: (count) => this.#countMutations(count),
      warn: (message) => this.#warn(message),
    });
    this.#mounts.set(mountId, {
      contribution,
      receiver,
      init,
      dependencies,
      onPropose,
      state: nativeMountStates.starting,
      sent: false,
    });
    this.#send(mountId);
    return { mountId, receiver };
  }

  update(mountId: string, init: NativeMountInit) {
    const mount = this.#mounts.get(mountId);
    if (!mount) return;
    mount.init = init;
    if (mount.sent)
      void this.#thread?.imports.update(mountId, init).catch(() => {});
  }

  /** Lets the host replace the broker callbacks a mount was created with. */
  bind(
    mountId: string,
    dependencies: BrokerDependencies,
    onPropose?: (value: unknown) => void,
  ) {
    const mount = this.#mounts.get(mountId);
    if (!mount) return;
    mount.dependencies = dependencies;
    mount.onPropose = onPropose;
  }

  unmount(mountId: string) {
    const mount = this.#mounts.get(mountId);
    if (!mount) return;
    this.#mounts.delete(mountId);
    if (mount.sent) void this.#thread?.imports.unmount(mountId).catch(() => {});
    mount.receiver.dispose();
  }

  /**
   * Resolves once the Worker has handled everything sent to it before, so a
   * value it proposes in answer to an input's last change has arrived.
   */
  async settle() {
    await this.#thread?.imports.ping().catch(() => {});
  }

  /** Stops the Worker; every mount shows the host failure state. */
  terminate(error: NativeUiErrorCode = nativeUiErrorCodes.unresponsive) {
    if (this.#error) return;
    this.#error = error;
    this.#timers.forEach((timer) => window.clearTimeout(timer));
    this.#abort.abort();
    this.isolation.stop(this.key);
    this.#mounts.forEach((mount) => mount.receiver.dispose());
    this.#notify();
  }

  #notify() {
    this.#listeners.forEach((listener) => listener());
  }

  #warn(message: string) {
    if (!development || this.#warned.has(message)) return;
    this.#warned.add(message);
    console.warn(`[${this.extensionName}] native UI: ${message}`);
  }

  #countNodes(delta: number) {
    this.#nodes += delta;
    if (this.#nodes > nativeUiBudgets.maximumTreeNodes) {
      this.terminate(nativeUiErrorCodes.treeTooLarge);
      throw new Error(nativeUiErrorCodes.treeTooLarge);
    }
  }

  #countMutations(count: number) {
    const now = performance.now();
    if (now - this.#windowStart > nativeUiBudgets.mutationWindow) {
      this.#windowStart = now;
      this.#windowMutations = 0;
    }
    this.#windowMutations += count;
    if (this.#windowMutations > nativeUiBudgets.maximumMutationsPerWindow) {
      this.terminate(nativeUiErrorCodes.mutationRateExceeded);
      throw new Error(nativeUiErrorCodes.mutationRateExceeded);
    }
  }

  /** The port, wrapped so every incoming message is measured first. */
  #messages(port: MessagePort): ThreadMessageTarget {
    return {
      send: (message, transferables) =>
        port.postMessage(message, transferables ?? []),
      listen: (listener, { signal }) => {
        port.addEventListener(
          'message',
          ({ data }) => {
            if (
              messageBytes(data, nativeUiBudgets.maximumMessageBytes) >
              nativeUiBudgets.maximumMessageBytes
            ) {
              this.terminate(nativeUiErrorCodes.messageTooLarge);
              return;
            }
            listener(data);
          },
          { signal },
        );
        port.start();
      },
    };
  }

  async #start(contribution: ExtensionContribution) {
    let artifact: string;
    try {
      artifact = await getExtensionArtifact(
        contribution.extension_id,
        contribution.id,
      );
    } catch {
      this.terminate(nativeUiErrorCodes.startFailed);
      return;
    }
    if (this.#error) return;
    const channel = new MessageChannel();
    // Only the host's exports are callable; a plain object would also expose
    // its prototype's methods by name.
    const exports: HostExports = Object.assign(Object.create(null), {
      ready: async (registered: unknown) => this.#ready(registered),
      request: async (mountId: unknown, method: unknown, payload: unknown) =>
        this.#request(mountId, method, payload),
    });
    this.#thread = new Thread<WorkerExports, HostExports>(
      this.#messages(channel.port1),
      {
        exports,
        signal: this.#abort.signal,
      },
    );
    this.#timers.push(
      window.setTimeout(() => {
        if (!this.#registered) this.terminate(nativeUiErrorCodes.startTimedOut);
      }, nativeUiBudgets.startTimeout),
    );
    await this.isolation.start(this.key, artifact, channel.port2, () =>
      this.terminate(
        this.#registered
          ? nativeUiErrorCodes.unresponsive
          : nativeUiErrorCodes.startFailed,
      ),
    );
  }

  #ready(registered: unknown) {
    if (this.#registered || this.#error) return;
    const ids = Array.isArray(registered)
      ? registered.filter((id): id is string => typeof id === 'string')
      : [];
    for (const id of ids)
      if (!this.#declared.has(id))
        this.#warn(
          `'${id}' is not a declared native contribution; its registration was rejected`,
        );
    this.#registered = new Set(ids.filter((id) => this.#declared.has(id)));
    this.#heartbeat();
    this.#mounts.forEach((_, mountId) => this.#send(mountId));
    this.#notify();
  }

  #heartbeat() {
    const timer = window.setTimeout(async () => {
      const answered = await Promise.race([
        this.#thread?.imports.ping().then(
          () => true,
          () => false,
        ),
        new Promise<false>((resolve) =>
          window.setTimeout(
            () => resolve(false),
            nativeUiBudgets.heartbeatTimeout,
          ),
        ),
      ]);
      if (this.#error) return;
      if (!answered) this.terminate(nativeUiErrorCodes.unresponsive);
      else this.#heartbeat();
    }, nativeUiBudgets.heartbeatInterval);
    this.#timers.push(timer);
  }

  #send(mountId: string) {
    const mount = this.#mounts.get(mountId);
    if (!mount || mount.sent || !this.#registered || !this.#thread) return;
    if (!this.#registered.has(mount.contribution.id)) {
      mount.state = nativeMountStates.failed;
      mount.error = nativeUiErrorCodes.notRegistered;
      this.#notify();
      return;
    }
    mount.sent = true;
    this.#thread.imports
      .mount(
        mountId,
        mount.contribution.id,
        mount.receiver.connection,
        mount.init,
      )
      .then(
        () => {
          mount.state = nativeMountStates.ready;
          this.#notify();
        },
        () => {
          mount.state = nativeMountStates.failed;
          mount.error = nativeUiErrorCodes.renderFailed;
          this.#notify();
        },
      );
  }

  async #request(mountId: unknown, method: unknown, payload: unknown) {
    const mount =
      typeof mountId === 'string' ? this.#mounts.get(mountId) : undefined;
    if (!mount || typeof method !== 'string')
      throw new Error(extensionProtocolErrors.requestDenied);
    try {
      if (method === extensionBrokerMethods.editorPropose) {
        if (
          mount.contribution.outlet !== extensionOutlets.attributeEditor ||
          !mount.onPropose ||
          !payload ||
          typeof payload !== 'object' ||
          !('value' in payload)
        )
          throw new Error(extensionProtocolErrors.requestDenied);
        mount.onPropose((payload as { value: unknown }).value);
        return null;
      }
      const data = await handleBrokerRequest(
        { method, payload },
        mount.dependencies,
      );
      if (jsonByteLength(data) > maximumExtensionResponseBytes)
        throw new Error();
      return data;
    } catch {
      // The reason never reaches extension code.
      throw new Error(extensionProtocolErrors.requestDenied);
    }
  }
}

const hosts = new Map<string, NativeExtensionHost>();

/** The Worker host for a native contribution's release, started on first use. */
export const nativeHostFor = (
  contribution: ExtensionContribution,
  runtime: ExtensionContribution[],
) => {
  const key = nativeHostKey(contribution);
  const declared = runtime.filter(
    (item) =>
      item.rendering === renderingModes.native && nativeHostKey(item) === key,
  );
  const existing = hosts.get(key);
  if (existing) {
    existing.declare(declared.length ? declared : [contribution]);
    return existing;
  }
  const host = new NativeExtensionHost(
    declared.length ? declared : [contribution],
  );
  hosts.set(key, host);
  return host;
};

/**
 * Terminates every Worker whose release is no longer in the runtime: disabled,
 * ungranted, upgraded, rebuilt, or contained by the deployment or workspace
 * gate. Only an unscoped runtime read decides this.
 */
export const containNativeHosts = (runtime: ExtensionContribution[]) => {
  const live = new Set(
    runtime
      .filter((item) => item.rendering === renderingModes.native)
      .map(nativeHostKey),
  );
  for (const [key, host] of hosts)
    if (!live.has(key)) {
      host.terminate();
      hosts.delete(key);
    }
};

/**
 * Starts the Worker of every enabled native release as soon as the app knows
 * them, so the shared sandbox frame and each Worker load once per app start
 * and a contribution mounted later waits only for its own data.
 */
export const startNativeHosts = (runtime: ExtensionContribution[]) => {
  const started = new Set<string>();
  for (const item of runtime) {
    if (item.rendering !== renderingModes.native) continue;
    const key = nativeHostKey(item);
    if (started.has(key)) continue;
    started.add(key);
    nativeHostFor(item, runtime);
  }
};
