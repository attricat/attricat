import { useEffect, useRef } from 'react';

// Fields that take a value. Checkboxes and radios are choices in a list, such
// as column preferences, rather than the start of a form. MUI selects are
// focusable comboboxes whose native inputs are hidden and skipped.
const fieldSelector = [
  'input:not([type="hidden"], [type="file"], [type="checkbox"], [type="radio"])',
  'textarea',
  'select',
  '[role="combobox"]',
].join(', ');

const canFocus = (element: HTMLElement) =>
  !element.matches(
    ':disabled, [readonly], [aria-disabled="true"], [tabindex="-1"]',
  ) && !element.closest('[hidden], [aria-hidden="true"]');

const firstField = (root: ParentNode | null | undefined) =>
  [...(root?.querySelectorAll<HTMLElement>(fieldSelector) ?? [])].find(
    canFocus,
  );

/**
 * Focuses the first field in `root` the user can change, preferring one inside
 * an element matching `prefer`; returns whether one was focused.
 */
export const focusFirstField = (
  root: ParentNode | null | undefined,
  prefer?: string,
) => {
  const preferred = prefer
    ? [...(root?.querySelectorAll(prefer) ?? [])].map(firstField).find(Boolean)
    : undefined;
  const field = preferred ?? firstField(root);
  field?.focus();
  return field !== undefined;
};

/**
 * Focuses the first field in `container`, or its first enabled button when it
 * has none, such as a file field, and scrolls it to the middle of the screen
 * so a sticky action bar does not cover it.
 */
export const focusControlIn = (container: ParentNode | null | undefined) => {
  const control =
    firstField(container) ??
    container?.querySelector<HTMLElement>('button:not(:disabled)');
  if (!control) return false;
  control.focus({ preventScroll: true });
  // Not implemented by every environment, such as jsdom.
  control.scrollIntoView?.({ block: 'center' });
  return true;
};

/**
 * Focuses the first field of a form as soon as one has rendered. Pass `null`
 * while the form is not ready; a new key focuses again, such as after another
 * blueprint loads. It never takes focus back once the user has clicked or
 * typed. Dialogs focus their first field through the theme instead.
 */
export const useInitialFocus = <T extends HTMLElement>(
  key: string | null = '',
  prefer?: string,
) => {
  const ref = useRef<T>(null);
  // The key whose fields were focused, or that the user acted on first.
  const handled = useRef<string | null>(null);
  useEffect(() => {
    if (key === null || handled.current === key) return;
    const stop = () => {
      handled.current = key;
    };
    document.addEventListener('pointerdown', stop, true);
    document.addEventListener('keydown', stop, true);
    return () => {
      document.removeEventListener('pointerdown', stop, true);
      document.removeEventListener('keydown', stop, true);
    };
  }, [key]);
  // Fields can render after the form is ready, such as when they wait for
  // their own data, so every render retries until one is focused.
  useEffect(() => {
    if (key === null || handled.current === key) return;
    if (focusFirstField(ref.current, prefer)) handled.current = key;
  });
  return ref;
};
