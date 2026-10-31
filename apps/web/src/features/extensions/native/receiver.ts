import {
  MUTATION_TYPE_INSERT_CHILD,
  MUTATION_TYPE_REMOVE_CHILD,
  MUTATION_TYPE_UPDATE_PROPERTY,
  MUTATION_TYPE_UPDATE_TEXT,
  NODE_TYPE_COMMENT,
  NODE_TYPE_ELEMENT,
  NODE_TYPE_TEXT,
  ROOT_ID,
  UPDATE_PROPERTY_TYPE_ATTRIBUTE,
  UPDATE_PROPERTY_TYPE_EVENT_LISTENER,
  type RemoteConnection,
  type RemoteMutationRecord,
  type RemoteNodeSerialization,
} from '@remote-dom/core';
import {
  uiKitElements,
  uiKitElementsByTag,
  uiKitLimits,
  type UiKitElementName,
} from './generated/uiKit';

/** The only attribute an element may carry: the slot it fills in its parent. */
const slotAttribute = 'slot';

type Listener = (payload: unknown) => unknown;

export type NativeTextNode = {
  kind: 'text';
  id: string;
  text: string;
};
export type NativeElementNode = {
  kind: 'element';
  id: string;
  /** Undefined when the tag is not a UI kit element of this version. */
  name: UiKitElementName | undefined;
  props: Record<string, unknown>;
  listeners: Record<string, Listener>;
  slot: string | undefined;
  children: NativeNode[];
};
type NativeIgnoredNode = { kind: 'ignored'; id: string };
export type NativeNode = NativeTextNode | NativeElementNode | NativeIgnoredNode;
export type NativeRootNode = Pick<NativeElementNode, 'id' | 'children'> & {
  kind: 'root';
};

/** What the receiver asks of the extension's host: budgets and diagnostics. */
export type ReceiverBudget = {
  /** Records nodes entering (positive) or leaving (negative) the tree. */
  countNodes: (delta: number) => void;
  /** Records mutation records received. */
  countMutations: (count: number) => void;
  /** Reports something the receiver dropped, in development. */
  warn: (message: string) => void;
};

/** `1.2.0` ≤ `1.10.0`, by SemVer precedence of release versions. */
export const versionAtMost = (version: string, limit: string) => {
  const parse = (value: string) => value.split('.').map(Number);
  const [left, right] = [parse(version), parse(limit)];
  for (let index = 0; index < 3; index += 1)
    if (left[index] !== right[index]) return left[index] < right[index];
  return true;
};

const definitionOf = (name: UiKitElementName) =>
  uiKitElements[name] as unknown as {
    since: string;
    props: Record<
      string,
      {
        since: string;
        required: boolean;
        default: unknown;
        schema: { safeParse: (value: unknown) => { success: boolean } };
      }
    >;
    events: Record<
      string,
      {
        since: string;
        payload: { safeParse: (value: unknown) => { success: boolean } };
      }
    >;
    children: { kind: 'none' | 'text' | 'elements' };
    slots: Record<string, readonly string[]>;
    methods: readonly string[];
    accessibility: readonly string[];
  };

/** An element's definition, when the negotiated UI kit version has it. */
export const elementDefinition = (name: UiKitElementName | undefined) =>
  name === undefined ? undefined : definitionOf(name);

/**
 * Receives one mounted contribution's remote tree. It keeps every node the
 * extension sent, so child indexes stay aligned, but validates each element,
 * prop, listener and text against the negotiated UI kit version; what fails
 * is kept out of what the host draws. Components subscribe per node.
 */
export class NativeReceiver {
  readonly root: NativeRootNode = { kind: 'root', id: ROOT_ID, children: [] };
  readonly connection: RemoteConnection;
  #nodes = new Map<string, NativeNode | NativeRootNode>([[ROOT_ID, this.root]]);
  #parents = new Map<string, string>();
  #versions = new Map<string, number>();
  #listeners = new Map<string, Set<() => void>>();
  #methods = new Map<string, Record<string, () => void>>();
  #disposed = false;

  constructor(
    readonly uiKit: string,
    readonly budget: ReceiverBudget,
  ) {
    this.connection = {
      mutate: (records) => this.mutate(records),
      call: (id, method) => this.call(id, method),
    };
  }

  mutate(records: readonly RemoteMutationRecord[]) {
    if (this.#disposed || !Array.isArray(records)) return;
    try {
      this.budget.countMutations(records.length);
      for (const record of records) this.#apply(record);
    } catch {
      // A budget was exceeded: the host has already stopped the extension's
      // Worker and disposed this tree, so the rest is dropped.
    }
  }

  #apply(record: RemoteMutationRecord) {
    if (!Array.isArray(record) || this.#disposed) return;
    switch (record[0]) {
      case MUTATION_TYPE_INSERT_CHILD:
        this.#insert(record[1], record[2], record[3]);
        break;
      case MUTATION_TYPE_REMOVE_CHILD:
        this.#remove(record[1], record[2]);
        break;
      case MUTATION_TYPE_UPDATE_TEXT:
        this.#updateText(record[1], record[2]);
        break;
      case MUTATION_TYPE_UPDATE_PROPERTY:
        this.#updateProperty(record[1], record[2], record[3], record[4]);
        break;
    }
  }

  /** Runs one of an element's declared host methods, such as `focus`. */
  call(id: string, method: string) {
    const node = this.#nodes.get(id);
    const allowed =
      node?.kind === 'element' &&
      elementDefinition(node.name)?.methods.includes(method);
    if (!allowed) throw new Error(`Method '${method}' is not available`);
    this.#methods.get(id)?.[method]?.();
  }

  /** Lets an adapter receive the methods its element declares. */
  registerMethods(id: string, methods: Record<string, () => void>) {
    this.#methods.set(id, methods);
    return () => {
      if (this.#methods.get(id) === methods) this.#methods.delete(id);
    };
  }

  subscribe(id: string, listener: () => void) {
    const listeners = this.#listeners.get(id) ?? new Set();
    listeners.add(listener);
    this.#listeners.set(id, listeners);
    return () => listeners.delete(listener);
  }

  version(id: string) {
    return this.#versions.get(id) ?? 0;
  }

  node(id: string) {
    return this.#nodes.get(id);
  }

  /**
   * Sends an event to the extension with a payload the host built. Only a
   * declared event with a listener and a payload matching its schema goes.
   */
  emit(node: NativeElementNode, event: string, payload: unknown) {
    if (this.#disposed) return;
    const listener = node.listeners[event];
    const definition = elementDefinition(node.name)?.events[event];
    if (!listener || !definition?.payload.safeParse(payload).success) return;
    void Promise.resolve(listener(payload)).catch(() => {});
  }

  /** Drops the tree and every listener the extension handed over. */
  dispose() {
    this.#disposed = true;
    this.budget.countNodes(-(this.#nodes.size - 1));
    this.#nodes.clear();
    this.#parents.clear();
    this.#methods.clear();
    this.root.children = [];
    this.#notify(ROOT_ID);
    this.#listeners.clear();
  }

  #notify(id: string | undefined) {
    if (id === undefined) return;
    this.#versions.set(id, this.version(id) + 1);
    this.#listeners.get(id)?.forEach((listener) => listener());
  }

  #parentOf(node: NativeNode | NativeRootNode) {
    return node.kind === 'root' ? undefined : this.#parents.get(node.id);
  }

  #build(serialization: RemoteNodeSerialization, parentId: string): NativeNode {
    const id = String(serialization.id);
    this.#parents.set(id, parentId);
    let node: NativeNode;
    if (serialization.type === NODE_TYPE_TEXT) {
      node = { kind: 'text', id, text: this.#text(serialization.data) };
    } else if (serialization.type === NODE_TYPE_ELEMENT) {
      const name = this.#elementName(serialization.element);
      node = {
        kind: 'element',
        id,
        name,
        props: {},
        listeners: {},
        slot: undefined,
        children: [],
      };
      for (const [property, value] of Object.entries(
        serialization.properties ?? {},
      ))
        this.#setProperty(node, property, value);
      for (const [attribute, value] of Object.entries(
        serialization.attributes ?? {},
      ))
        this.#setAttribute(node, attribute, value);
      for (const [event, listener] of Object.entries(
        serialization.eventListeners ?? {},
      ))
        this.#setListener(node, event, listener);
      this.#nodes.set(id, node);
      node.children = (serialization.children ?? []).map((child) =>
        this.#build(child, id),
      );
    } else {
      if (serialization.type !== NODE_TYPE_COMMENT)
        this.budget.warn('an unsupported node type was dropped');
      node = { kind: 'ignored', id };
    }
    this.#nodes.set(id, node);
    this.budget.countNodes(1);
    return node;
  }

  #elementName(tag: unknown) {
    const name = typeof tag === 'string' ? uiKitElementsByTag[tag] : undefined;
    if (!name || !versionAtMost(uiKitElements[name].since, this.uiKit)) {
      this.budget.warn(
        `<${String(tag)}> is not a UI kit ${this.uiKit} element and was dropped`,
      );
      return undefined;
    }
    return name;
  }

  #text(value: unknown) {
    if (typeof value !== 'string') return '';
    if (value.length <= uiKitLimits.text) return value;
    this.budget.warn(`text longer than ${uiKitLimits.text} was shortened`);
    return value.slice(0, uiKitLimits.text);
  }

  #setProperty(node: NativeElementNode, property: string, value: unknown) {
    const definition = elementDefinition(node.name)?.props[property];
    if (value === undefined || value === null) {
      // Clearing a prop restores its default; `null` is a value only where
      // the schema allows it.
      if (value === null && definition?.schema.safeParse(null).success)
        node.props[property] = null;
      else delete node.props[property];
      return;
    }
    if (
      !definition ||
      !versionAtMost(definition.since, this.uiKit) ||
      !definition.schema.safeParse(value).success
    ) {
      this.budget.warn(
        `${node.name ?? 'element'}.${property} was dropped: it is undeclared or invalid`,
      );
      delete node.props[property];
      return;
    }
    node.props[property] = value;
  }

  #setAttribute(node: NativeElementNode, attribute: string, value: unknown) {
    if (attribute !== slotAttribute) {
      this.budget.warn(`attribute '${attribute}' was dropped`);
      return;
    }
    node.slot = typeof value === 'string' ? value : undefined;
  }

  #setListener(node: NativeElementNode, event: string, listener: unknown) {
    const definition = elementDefinition(node.name)?.events[event];
    if (typeof listener !== 'function') {
      delete node.listeners[event];
      return;
    }
    if (!definition || !versionAtMost(definition.since, this.uiKit)) {
      this.budget.warn(`${node.name ?? 'element'} has no '${event}' event`);
      return;
    }
    node.listeners[event] = listener as Listener;
  }

  #insert(parentId: string, child: RemoteNodeSerialization, index: number) {
    const parent = this.#nodes.get(parentId);
    if (!parent || (parent.kind !== 'element' && parent.kind !== 'root'))
      return;
    const node = this.#build(child, parentId);
    const at = Number.isInteger(index)
      ? Math.max(0, Math.min(index, parent.children.length))
      : parent.children.length;
    parent.children.splice(at, 0, node);
    this.#notify(parentId);
  }

  #remove(parentId: string, index: number) {
    const parent = this.#nodes.get(parentId);
    if (!parent || (parent.kind !== 'element' && parent.kind !== 'root'))
      return;
    const [removed] = parent.children.splice(index, 1);
    if (removed) this.#forget(removed);
    this.#notify(parentId);
  }

  #forget(node: NativeNode) {
    this.#nodes.delete(node.id);
    this.#parents.delete(node.id);
    this.#methods.delete(node.id);
    this.#listeners.delete(node.id);
    this.budget.countNodes(-1);
    if (node.kind === 'element')
      node.children.forEach((child) => this.#forget(child));
  }

  #updateText(id: string, text: string) {
    const node = this.#nodes.get(id);
    if (node?.kind !== 'text') return;
    node.text = this.#text(text);
    this.#notify(this.#parentOf(node));
  }

  #updateProperty(id: string, name: string, value: unknown, type?: number) {
    const node = this.#nodes.get(id);
    if (node?.kind !== 'element') return;
    if (type === UPDATE_PROPERTY_TYPE_EVENT_LISTENER)
      this.#setListener(node, name, value);
    else if (type === UPDATE_PROPERTY_TYPE_ATTRIBUTE) {
      this.#setAttribute(node, name, value);
      // A slot change moves the element within its parent's drawing.
      this.#notify(this.#parentOf(node));
    } else this.#setProperty(node, name, value);
    this.#notify(id);
  }
}

/**
 * Whether an element can be drawn: a known element whose required props and
 * accessible names are present. The host draws nothing in its place
 * otherwise.
 */
export const drawable = (node: NativeElementNode) => {
  const definition = elementDefinition(node.name);
  if (!definition) return false;
  for (const [prop, item] of Object.entries(definition.props))
    if (item.required && node.props[prop] === undefined) return false;
  return definition.accessibility.every((prop) => {
    const value = node.props[prop];
    return typeof value === 'string' && value.trim() !== '';
  });
};

/** A parent's drawable children of the kind it accepts, and its slots. */
export const drawnChildren = (node: NativeElementNode | NativeRootNode) => {
  const accepts =
    node.kind === 'root'
      ? 'elements'
      : (elementDefinition(node.name)?.children.kind ?? 'none');
  const slots =
    node.kind === 'root' ? {} : (elementDefinition(node.name)?.slots ?? {});
  const children: NativeNode[] = [];
  const slotted: Record<string, NativeElementNode> = {};
  for (const child of node.children) {
    if (child.kind === 'ignored') continue;
    if (child.kind === 'element' && child.slot !== undefined) {
      const allowed = slots[child.slot];
      if (
        allowed &&
        child.name &&
        allowed.includes(child.name) &&
        !slotted[child.slot]
      )
        slotted[child.slot] = child;
      continue;
    }
    if (accepts === 'text' && child.kind === 'text') children.push(child);
    if (accepts === 'elements' && child.kind === 'element')
      children.push(child);
  }
  return { children, slots: slotted };
};
