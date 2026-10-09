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
const form = (canWrite: boolean, notes?: string, entitySchema?: unknown) =>
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
        entity_schema: entitySchema,
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
      ...(notes === undefined
        ? []
        : [
            {
              kind: 'scalar',
              attribute_code: 'notes',
              context_id: contextId,
              value: notes,
            },
          ]),
    ],
    reusable_attributes: [],
    reusable_values: [],
    context: {},
  }) as unknown as EntityFormResponse;
const resolved = {
  name: { value: 'Lamp', source_context: { id: contextId, code: 'default' } },
  sku: { value: 'L-1', source_context: { id: contextId, code: 'default' } },
};

const renderFields = (
  canWrite = true,
  notes?: string,
  {
    entitySchema,
    viewContextId = contextId,
  }: {
    entitySchema?: unknown;
    viewContextId?: string;
  } = {},
) => {
  const ref = createRef<EntityInlineFieldsHandle>();
  render(
    <QueryClientProvider client={new QueryClient()}>
      <EntityInlineFields
        attributes={attributes}
        contextId={viewContextId}
        defaultContextId={contextId}
        entityId={entityId}
        form={form(canWrite, notes, entitySchema)}
        ref={ref}
        resolvedValues={
          notes === undefined
            ? resolved
            : {
                ...resolved,
                notes: {
                  value: notes,
                  source_context: { id: contextId, code: 'default' },
                },
              }
        }
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

  it('shows set Markdown rendered until the user asks to edit it', async () => {
    const user = userEvent.setup();
    renderFields(true, '**Bright** light');

    expect(screen.getByText('Bright').tagName).toBe('STRONG');
    expect(screen.queryByRole('tab', { name: 'Write' })).toBeNull();

    await user.click(screen.getByRole('button', { name: 'Edit notes' }));
    const notes = screen.getByRole('textbox', { name: 'notes' });
    expect(document.activeElement).toBe(notes);
    expect((notes as HTMLTextAreaElement).selectionStart).toBe(
      '**Bright** light'.length,
    );
    await user.type(notes, ' and *warm*');
    await user.click(screen.getByRole('button', { name: 'Elsewhere' }));

    await waitFor(() => expect(updateEntity).toHaveBeenCalledOnce());
    expect(vi.mocked(updateEntity).mock.calls[0]![1].values).toEqual([
      {
        kind: 'scalar',
        attribute_code: 'notes',
        context_id: contextId,
        value: '**Bright** light and *warm*',
      },
    ]);
    expect(screen.getByText('warm').tagName).toBe('EM');
    expect(screen.queryByRole('textbox', { name: 'notes' })).toBeNull();
  });

  it('keeps the Markdown editor open while switching to its preview', async () => {
    const user = userEvent.setup();
    renderFields(true, 'Bright');

    await user.click(screen.getByRole('button', { name: 'Edit notes' }));
    await user.type(screen.getByRole('textbox', { name: 'notes' }), '!');
    await user.click(screen.getByRole('tab', { name: 'Preview' }));

    expect(screen.getByRole('tab', { name: 'Write' })).toBeTruthy();
    expect(updateEntity).not.toHaveBeenCalled();
  });

  it('closes the Markdown editor on Escape without saving', async () => {
    const user = userEvent.setup();
    renderFields(true, 'Bright');

    await user.click(screen.getByRole('button', { name: 'Edit notes' }));
    await user.type(screen.getByRole('textbox', { name: 'notes' }), '!');
    await user.keyboard('{Escape}');

    expect(screen.queryByRole('textbox', { name: 'notes' })).toBeNull();
    expect(screen.getByText('Bright')).toBeTruthy();
    expect(document.activeElement).toBe(
      screen.getByRole('button', { name: 'Edit notes' }),
    );
    expect(updateEntity).not.toHaveBeenCalled();
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

  it('marks fields the entity schema requires', () => {
    renderFields(true, 'Hello', {
      entitySchema: { required: ['name', 'notes'] },
    });

    expect(
      (screen.getByRole('textbox', { name: 'name' }) as HTMLInputElement)
        .required,
    ).toBe(true);
    // A field shown as its value marks its label until it is edited.
    expect(screen.getByText('notes').textContent).toBe('notes *');
  });

  it('does not mark required fields outside the default context', () => {
    renderFields(true, undefined, {
      entitySchema: { required: ['name'] },
      viewContextId: '123e4567-e89b-12d3-a456-426614174002',
    });

    expect(
      (screen.getByRole('textbox', { name: 'name' }) as HTMLInputElement)
        .required,
    ).toBe(false);
  });

  it('keeps a required field that was cleared unsaved', async () => {
    const user = userEvent.setup();
    renderFields(true, undefined, { entitySchema: { required: ['name'] } });

    await user.clear(screen.getByRole('textbox', { name: 'name' }));
    await user.click(screen.getByRole('button', { name: 'Elsewhere' }));

    expect(await screen.findByText('A value is required.')).toBeTruthy();
    expect(updateEntity).not.toHaveBeenCalled();
  });

  it('saves a cleared required field outside the default context', async () => {
    const user = userEvent.setup();
    const otherContextId = '123e4567-e89b-12d3-a456-426614174002';
    renderFields(true, undefined, {
      entitySchema: { required: ['name'] },
      viewContextId: otherContextId,
    });
    const name = screen.getByRole('textbox', { name: 'name' });

    await user.type(name, 'Lamp');
    await user.click(screen.getByRole('button', { name: 'Elsewhere' }));
    await waitFor(() => expect(updateEntity).toHaveBeenCalledOnce());
    await user.clear(name);
    await user.click(screen.getByRole('button', { name: 'Elsewhere' }));

    await waitFor(() => expect(updateEntity).toHaveBeenCalledTimes(2));
    expect(vi.mocked(updateEntity).mock.calls[1]![1].remove_values).toEqual([
      { attribute_code: 'name', context_id: otherContextId },
    ]);
    expect(screen.queryByText('A value is required.')).toBeNull();
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
