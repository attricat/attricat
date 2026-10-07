import { Box } from '@mui/material';
import {
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
  children: (props: {
    value: string;
    error?: string;
    onChange: (value: string) => void;
  }) => ReactNode;
};

/**
 * Hosts one always-editable field. Typing stays local until focus leaves the
 * field or Enter is pressed in a single-line input; choices made without
 * typing commit at once. Escape returns the field to its saved value.
 */
export const InlineFieldEditor = ({
  value,
  immediate = false,
  validate,
  onCommit,
  onRevert,
  children,
}: Props) => {
  const [draft, setDraft] = useState<string | null>(null);
  const [error, setError] = useState<string>();
  const shown = draft ?? value;

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
      onBlur={(event: FocusEvent<HTMLDivElement>) => {
        if (draft === null) return;
        if (event.currentTarget.contains(event.relatedTarget as Node | null))
          return;
        commit(draft);
      }}
      onKeyDown={(event: KeyboardEvent<HTMLDivElement>) => {
        if (!isTextEntry(event.target as Element)) return;
        if (event.key === 'Escape') {
          setDraft(null);
          setError(undefined);
          onRevert();
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
      {children({
        value: shown,
        error,
        onChange: (next) => {
          if (immediate || !isTextEntry(document.activeElement)) commit(next);
          else {
            setError(undefined);
            setDraft(next);
          }
        },
      })}
    </Box>
  );
};
