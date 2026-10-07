// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { createRef } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import { updateEntity, type Attribute, type EntityFormResponse } from '../api';
import { VIEW_COMPONENT_IDS } from '../../views/constants';
import { entityHeadingComponentId } from '../../views/components/blocks/EntityHeadingDefinition';
import {
  EntityInlineFields,
  type EntityInlineFieldsHandle,
} from './EntityInlineFields';

vi.mock('../api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api')>()),
  updateEntity: vi.fn(),
}));

const entityId = '123e4567-e89b-12d3-a456-426614174010';
const contextId = '123e4567-e89b-12d3-a456-426614174001';
const attributes: Attribute[] = [
  { code: 'name', value_type: 'string' },
  { code: 'notes', value_type: 'string' },
  { code: 'sku', value_type: 'string', readonly: true },
];
const view = {
  type: 'stack' as const,
  children: [
    {
      type: 'stack' as const,
      component: { id: entityHeadingComponentId, version: 1, props: {} },
      children: [{ type: 'field' as const, field: 'name' }],
    },
    {
      type: 'field' as const,
      field: 'notes',
      component: {
        id: VIEW_COMPONENT_IDS.markdownDisplay,
        version: 1,
        props: {},
      },
    },
    { type: 'field' as const, field: 'sku' },
  ],
};
const form = (canWrite: boolean) =>
  ({
    can_write: canWrite,
    entity: {
      id: entityId,
      blueprint_id: '123e4567-e89b-12d3-a456-426614174000',
      blueprint_version: 1,
      updated_at: '2026-10-07T10:00:00Z',
    },
    blueprint: {
      blueprint: {
        id: '123e4567-e89b-12d3-a456-426614174000',
        code: 'product',
        name: 'Product',
        status: 'published',
        version: 1,
        views: { detail: view },
      },
      attributes,
      table_path_attributes: [],
    },
    values: [
      {
        kind: 'scalar',
        attribute_code: 'name',
        context_id: contextId,
        value: 'Lamp',
      },
      {
        kind: 'scalar',
        attribute_code: 'sku',
        context_id: contextId,
        value: 'L-1',
      },
    ],
    reusable_attributes: [],
    reusable_values: [],
    context: {},
  }) as unknown as EntityFormResponse;
const resolved = {
  name: { value: 'Lamp', source_context: { id: contextId, code: 'default' } },
  sku: { value: 'L-1', source_context: { id: contextId, code: 'default' } },
};

const renderFields = (canWrite = true) => {
  const ref = createRef<EntityInlineFieldsHandle>();
  render(
    <QueryClientProvider client={new QueryClient()}>
      <EntityInlineFields
        attributes={attributes}
        contextId={contextId}
        defaultContextId={contextId}
        entityId={entityId}
        form={form(canWrite)}
        ref={ref}
        resolvedValues={resolved}
        reusableResolvedValues={{}}
        statusParentContextIds={[]}
        view={view}
      />
      <button type="button">Elsewhere</button>
    </QueryClientProvider>,
  );
  return ref;
};

describe('EntityInlineFields', () => {
  beforeEach(() => {
    vi.mocked(updateEntity).mockReset();
    vi.mocked(updateEntity).mockResolvedValue({
      id: entityId,
      updated_at: '2026-10-07T10:01:00Z',
    } as never);
  });

  it('edits fields in the detail layout and saves one field on blur', async () => {
    const user = userEvent.setup();
    renderFields();

    // The heading only displays values, so its field is edited first.
    const name = screen.getByRole('textbox', { name: 'name' });
    await user.clear(name);
    await user.type(name, 'Desk lamp');
    await user.click(screen.getByRole('button', { name: 'Elsewhere' }));

    await waitFor(() => expect(updateEntity).toHaveBeenCalledOnce());
    expect(vi.mocked(updateEntity).mock.calls[0]![1]).toEqual({
      expected_updated_at: '2026-10-07T10:00:00Z',
      values: [
        {
          kind: 'scalar',
          attribute_code: 'name',
          context_id: contextId,
          value: 'Desk lamp',
        },
      ],
      relationships: [],
      remove_values: [],
    });
    // The markdown display component edits with its markdown editor.
    expect(screen.getByRole('tab', { name: 'Write' })).toBeTruthy();
  });

  it('does not mark a field while its save is in flight', async () => {
    const user = userEvent.setup();
    vi.mocked(updateEntity).mockReturnValue(new Promise(() => {}));
    renderFields();

    await user.type(screen.getByRole('textbox', { name: 'name' }), '!');
    await user.click(screen.getByRole('button', { name: 'Elsewhere' }));

    await waitFor(() => expect(screen.getByText('Saving…')).toBeTruthy());
    expect(screen.queryByText('This change is not saved yet.')).toBeNull();
  });

  it('shows values read-only to users who cannot edit', () => {
    renderFields(false);
    expect(screen.queryByRole('textbox')).toBeNull();
    expect(screen.getByText('L-1')).toBeTruthy();
    expect(screen.queryByText('Other attributes')).toBeNull();
  });

  it('keeps readonly attributes as values', () => {
    renderFields();
    expect(screen.queryByRole('textbox', { name: 'sku' })).toBeNull();
    expect(screen.getByText('L-1')).toBeTruthy();
  });

  it('saves applied Smart Fill suggestions together', async () => {
    const ref = renderFields();
    ref.current!.applySmartFillValues({
      name: 'Floor lamp',
      notes: 'Tall',
      sku: 'ignored',
    });

    await waitFor(() => expect(updateEntity).toHaveBeenCalledOnce());
    expect(
      vi
        .mocked(updateEntity)
        .mock.calls[0]![1].values.map((value) => value.attribute_code),
    ).toEqual(['name', 'notes']);
  });
});
