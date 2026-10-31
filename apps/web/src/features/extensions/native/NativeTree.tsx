import { useCallback, useSyncExternalStore } from 'react';
import { ROOT_ID } from '@remote-dom/core';
import { nativeAdapters } from './adapterMap';
import {
  drawable,
  drawnChildren,
  elementDefinition,
  type NativeElementNode,
  type NativeNode,
  type NativeReceiver,
} from './receiver';

/** Re-renders when the receiver changes this node or its children. */
const useNode = (receiver: NativeReceiver, id: string) =>
  useSyncExternalStore(
    useCallback((listener) => receiver.subscribe(id, listener), [receiver, id]),
    () => receiver.version(id),
  );

const NativeChildren = ({
  nodes,
  receiver,
}: {
  nodes: NativeNode[];
  receiver: NativeReceiver;
}) => {
  if (nodes.every((node) => node.kind === 'text'))
    return nodes
      .map((node) => (node.kind === 'text' ? node.text : ''))
      .join('');
  return nodes.map((node) =>
    node.kind === 'element' ? (
      <NativeElement key={node.id} node={node} receiver={receiver} />
    ) : null,
  );
};

const NativeElement = ({
  node,
  receiver,
}: {
  node: NativeElementNode;
  receiver: NativeReceiver;
}) => {
  useNode(receiver, node.id);
  const provideMethods = useCallback(
    (methods: Record<string, () => void>) => {
      receiver.registerMethods(node.id, methods);
    },
    [receiver, node.id],
  );
  const emit = useCallback(
    (event: string, payload: unknown) => receiver.emit(node, event, payload),
    [receiver, node],
  );
  const definition = elementDefinition(node.name);
  if (!definition || !node.name || !drawable(node)) return null;
  const Adapter = nativeAdapters[node.name];
  const props: Record<string, unknown> = {};
  for (const [prop, item] of Object.entries(definition.props)) {
    const value = node.props[prop];
    if (value !== undefined) props[prop] = value;
    else if (item.default !== null) props[prop] = item.default;
  }
  const { children, slots } = drawnChildren(node);
  return (
    <Adapter
      emit={emit}
      listens={(event) => Boolean(node.listeners[event])}
      props={props}
      provideMethods={provideMethods}
      slots={Object.fromEntries(
        Object.entries(slots).map(([slot, child]) => [
          slot,
          <NativeElement key={child.id} node={child} receiver={receiver} />,
        ]),
      )}
    >
      <NativeChildren nodes={children} receiver={receiver} />
    </Adapter>
  );
};

/** Draws a mounted contribution's validated tree with host components. */
export const NativeTree = ({ receiver }: { receiver: NativeReceiver }) => {
  useNode(receiver, ROOT_ID);
  return (
    <NativeChildren
      nodes={drawnChildren(receiver.root).children}
      receiver={receiver}
    />
  );
};
