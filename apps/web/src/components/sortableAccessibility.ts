import { Accessibility, type Draggable } from '@dnd-kit/dom';
import { isSortable } from '@dnd-kit/react/sortable';
import type { TFunction } from 'i18next';

// `Accessibility.configure` is untyped, so take its options from the plugin.
type AccessibilityOptions = NonNullable<
  ConstructorParameters<typeof Accessibility>[1]
>;

/**
 * dnd-kit's plugins with its English announcements of raw IDs replaced: each
 * message gets `position` and `count`, and what `describe` says about the item.
 */
export const sortablePlugins = ({
  count,
  describe,
  messages,
  t,
}: {
  count: number;
  describe: (id: string) => Record<string, string>;
  messages: {
    cancel: string;
    end: string;
    instructions: string;
    over: string;
    start: string;
  };
  t: TFunction;
}) => {
  const announce = (key: string, source: Draggable | null) =>
    source
      ? t(key, {
          ...describe(String(source.id)),
          position: isSortable(source) ? source.index + 1 : undefined,
          count,
        })
      : undefined;
  const options: AccessibilityOptions = {
    announcements: {
      dragstart: ({ operation: { source } }) =>
        announce(messages.start, source),
      dragover: ({ operation: { source } }) => announce(messages.over, source),
      dragend: ({ operation: { source }, canceled }) =>
        announce(canceled ? messages.cancel : messages.end, source),
    },
    screenReaderInstructions: { draggable: t(messages.instructions) },
  };
  const accessibility = Accessibility.configure(options);
  return <T>(defaults: T[]) =>
    defaults.map((plugin) =>
      plugin === Accessibility ? accessibility : plugin,
    );
};
