// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ComponentProps } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import type { Attribute, FormAttributeValue } from '../api';
import { RecordFormAttributeEditor } from './RecordFormAttributeEditor';

const defaultContextId = '123e4567-e89b-12d3-a456-426614174001';
const marketContextId = '123e4567-e89b-12d3-a456-426614174002';
const title: Attribute = {
  code: 'title',
  value_type: 'string',
  context_fallback: 'default',
};
const localTitle: FormAttributeValue = {
  kind: 'scalar',
  attribute_code: 'title',
  context_id: marketContextId,
  value: 'Koszula',
};

const renderEditor = (
  props: Partial<ComponentProps<typeof RecordFormAttributeEditor>> = {},
) => {
  const onChange = vi.fn();
  render(
    <QueryClientProvider client={new QueryClient()}>
      <RecordFormAttributeEditor
        attribute={title}
        contextId={marketContextId}
        defaultContextId={defaultContextId}
        existingValues={[localTitle]}
        fieldErrors={{}}
        onChange={onChange}
        resolvedValues={{}}
        value="Koszula"
        {...props}
      />
    </QueryClientProvider>,
  );
  return onChange;
};

afterEach(cleanup);

describe('RecordFormAttributeEditor', () => {
  it('lets a context value fall back to the inherited one', () => {
    const onChange = renderEditor();
    fireEvent.click(
      screen.getByRole('button', { name: 'Use inherited value' }),
    );
    expect(onChange).toHaveBeenCalledWith('');
  });

  it.each([
    ['in the default context', { contextId: defaultContextId }],
    ['without a value of its own', { existingValues: [] }],
    [
      'for an attribute that does not inherit',
      { attribute: { ...title, context_fallback: 'none' as const } },
    ],
    [
      'for a relationship, which an empty list would replace',
      { attribute: { ...title, value_type: 'relationship' as const } },
    ],
    ['while the field is locked', { lockedAttributes: { title: 'Published' } }],
  ])('offers no inherited value %s', (_, props) => {
    renderEditor(props);
    expect(
      screen.queryByRole('button', { name: 'Use inherited value' }),
    ).toBeNull();
  });

  it('discards a stored value a migration cannot carry over, and undoes it', () => {
    const onDiscardChange = vi.fn();
    const review = {
      message: 'The value type changed.',
      previousValue: 'large',
      fieldValue: 'large',
      onDiscardChange,
    };
    const onChange = renderEditor({
      contextId: defaultContextId,
      migrationReviews: { title: review },
    });
    fireEvent.click(
      screen.getByRole('button', { name: 'Discard previous value' }),
    );
    expect(onChange).toHaveBeenLastCalledWith('');
    expect(onDiscardChange).toHaveBeenLastCalledWith(true);

    cleanup();
    const undo = renderEditor({
      contextId: defaultContextId,
      migrationReviews: { title: { ...review, discarded: true } },
      value: '',
    });
    expect(
      screen.getByText('Previous value large will be archived.'),
    ).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Undo' }));
    expect(undo).toHaveBeenLastCalledWith('large');
    expect(onDiscardChange).toHaveBeenLastCalledWith(false);
  });

  it('keeps a protected value readable and says why it cannot change', () => {
    renderEditor({
      attribute: { ...title, readonly: true, description: 'From the import.' },
      contextId: defaultContextId,
    });
    const input = screen.getByRole('textbox', { name: 'title' });
    expect(input).toHaveProperty('disabled', true);
    expect(input).toHaveProperty('value', 'Koszula');
    expect(
      screen.getByText('Managed by system actions. From the import.'),
    ).toBeTruthy();
  });
});
