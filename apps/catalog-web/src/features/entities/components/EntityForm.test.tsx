// @vitest-environment jsdom
import { createRef } from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import type { BlueprintWithAttributes, Attribute } from '../api';
import { EntityForm, type EntityFormHandle } from './EntityForm';

const attribute = (
  code: string,
  overrides: Partial<Attribute> = {},
): Attribute => ({
  code,
  value_type: 'string',
  ...overrides,
});

const blueprint = (attributes: Attribute[]): BlueprintWithAttributes => ({
  blueprint: {
    id: '123e4567-e89b-12d3-a456-426614174000',
    code: 'product',
    name: 'Product',
    status: 'published',
    version: 1,
    views: {},
  },
  attributes,
  table_path_attributes: [],
});

const renderForm = (
  props: Partial<React.ComponentProps<typeof EntityForm>>,
) => {
  const ref = createRef<EntityFormHandle>();
  const onSubmit = vi.fn();
  render(
    <QueryClientProvider client={new QueryClient()}>
      <EntityForm
        blueprint={blueprint([])}
        contextId="123e4567-e89b-12d3-a456-426614174001"
        defaultContextId="123e4567-e89b-12d3-a456-426614174001"
        onSubmit={onSubmit}
        ref={ref}
        submitLabel="Save"
        {...props}
      />
    </QueryClientProvider>,
  );
  return { ref, onSubmit };
};

describe('EntityForm', () => {
  it('does not inject Smart Fill data into fields hidden by the default edit view', async () => {
    const { ref, onSubmit } = renderForm({
      blueprint: blueprint([
        attribute('title'),
        attribute('secret', { tags: ['hidden:form'] }),
        attribute('readonly', { readonly: true }),
      ]),
    });
    ref.current?.applySmartFillValues({
      title: 'New title',
      secret: 'Surprise',
      readonly: 'No',
    });
    await waitFor(() =>
      expect(screen.getByRole('textbox', { name: 'title' })).toHaveProperty(
        'value',
        'New title',
      ),
    );
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(onSubmit).toHaveBeenCalledOnce());
    expect(onSubmit.mock.calls[0][0].values).toEqual([
      expect.objectContaining({ attribute_code: 'title', value: 'New title' }),
    ]);
  });

  it('uses the same managed and inherited feedback for reusable attributes', () => {
    renderForm({
      blueprint: blueprint([]),
      contextId: '123e4567-e89b-12d3-a456-426614174002',
      reusableAttributes: [
        attribute('namespace:managed', { readonly: true }),
        attribute('namespace:inherited'),
      ],
      resolvedValues: {
        'namespace:inherited': {
          value: 'Original',
          source_context: {
            id: '123e4567-e89b-12d3-a456-426614174001',
            code: 'default',
          },
        },
      },
    });
    expect(screen.getByText('Managed by system actions')).toBeTruthy();
    expect(screen.getByText(/Original/)).toBeTruthy();
  });
});
