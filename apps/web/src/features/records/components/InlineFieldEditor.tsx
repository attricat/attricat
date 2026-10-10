import { Box } from '@mui/material';
import {
  useLayoutEffect,
  useRef,
  useState,
  type FocusEvent,
  type KeyboardEvent,
  type ReactNode,
} from 'react';

/** Input types where a change is part of typing rather than a final choice. */
const textEntryInputTypes = new Set([
  'date',
  'datetime-local',
  'email',
  'month',
  'number',
  'search',
  'tel',
  'text',
  'time',
  'url',
  'week',
]);

const isTextEntry = (element: Element | null): boolean =>
  element instanceof HTMLTextAreaElement ||
  (element instanceof HTMLElement && element.isContentEditable) ||
  (element instanceof HTMLInputElement &&
    textEntryInputTypes.has(element.type));

type Props = {
  /** The field's saved or pending value. */
  value: string;
  /** Commits every change at once, as for selects and relationship pickers. */
  immediate?: boolean;
  /** Returns why a value cannot be saved; it is then kept as a local edit. */
  validate?: (value: string) => string | undefined;
  onCommit: (value: string) => void;
  /** Discards the field's unsaved value, including a rejected pending one. */
  onRevert: () => void;
  /**
   * Shows a set value at rest until `onEdit` opens the editor, which closes
   * again once focus leaves it or Escape is pressed. The editor stays open
   * while its value is blank or not committed; Escape returns focus to the
   * first button at rest. Without it the field is always editable.
   */
  renderAtRest?: (props: { value: string; onEdit: () => void }) => ReactNode;
  children: (props: {
    value: string;
    error?: string;
    onChange: (value: string) => void;
  }) => ReactNode;
};

/**
 * Hosts one editable field, always editable unless it has a view at rest.
 * Typing stays local until focus leaves the
 * field or Enter is pressed in a single-line input; choices made without
 * typing commit at once. Escape returns the field to its saved value.
 */
export const InlineFieldEditor = ({
  value,
  immediate = false,
  validate,
  onCommit,
  onRevert,
  renderAtRest,
  children,
}: Props) => {
  const [draft, setDraft] = useState<string | null>(null);
  const [error, setError] = useState<string>();
  const [focused, setFocused] = useState(false);
  const [baseline, setBaseline] = useState(value);
  // A value set from outside (Smart Fill, a resolved conflict) replaces an
  // edit left behind in the field, but never text the user is typing.
  if (value !== baseline) {
    setBaseline(value);
    if (!focused) {
      setDraft(null);
      setError(undefined);
    }
  }
  const shown = draft ?? value;
  const [editing, setEditing] = useState(false);
  const container = useRef<HTMLDivElement>(null);
  const returnFocus = useRef(false);
  useLayoutEffect(() => {
    const root = container.current;
    if (editing) {
      const field = root?.querySelector<HTMLTextAreaElement | HTMLInputElement>(
        'textarea:not([aria-hidden]), input:not([type="hidden"])',
      );
      field?.focus();
      // Continue after the existing text, as when clicking below it.
      if (field instanceof HTMLTextAreaElement)
        field.setSelectionRange(field.value.length, field.value.length);
    } else if (returnFocus.current) root?.querySelector('button')?.focus();
    returnFocus.current = false;
  }, [editing]);
  const atRest =
    renderAtRest !== undefined &&
    !editing &&
    draft === null &&
    error === undefined &&
    value.trim() !== '';

  const commit = (next: string) => {
    const invalid = validate?.(next);
    setError(invalid);
    if (invalid) {
      setDraft(next);
      return;
    }
    setDraft(null);
    onCommit(next);
  };

  return (
    <Box
      ref={container}
      onFocus={() => setFocused(true)}
      onBlur={(event: FocusEvent<HTMLDivElement>) => {
        if (event.currentTarget.contains(event.relatedTarget as Node | null))
          return;
        // Switching between the value at rest and the editor removes the
        // focused element; focus has not left the field.
        if (renderAtRest && !(event.target as Node).isConnected) return;
        setFocused(false);
        setEditing(false);
        if (draft !== null) commit(draft);
      }}
      onKeyDown={(event: KeyboardEvent<HTMLDivElement>) => {
        if (!isTextEntry(event.target as Element)) return;
        if (event.key === 'Escape') {
          setDraft(null);
          setError(undefined);
          onRevert();
          if (editing) {
            setEditing(false);
            returnFocus.current = true;
          }
        } else if (
          event.key === 'Enter' &&
          event.target instanceof HTMLInputElement &&
          draft !== null
        ) {
          event.preventDefault();
          commit(draft);
        }
      }}
    >
      {atRest
        ? renderAtRest({
            value,
            onEdit: () => setEditing(true),
          })
        : children({
            value: shown,
            error,
            onChange: (next) => {
              if (immediate || !isTextEntry(document.activeElement))
                commit(next);
              else {
                setError(undefined);
                setDraft(next);
              }
            },
          })}
    </Box>
  );
};
