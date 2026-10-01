// @vitest-environment jsdom
import { createRef } from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import { authQueryKeys } from '../../auth/queryKeys';
import { draftEditors } from '../../drafts/constants';
import { draftStorageKey, writeDraft } from '../../drafts/draftStorage';
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
  const client = new QueryClient({
    defaultOptions: { queries: { staleTime: Infinity } },
  });
  client.setQueryData(authQueryKeys.session(), session);
  render(
    <QueryClientProvider client={client}>
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

const session = { user_id: 'user-1', workspace_id: 'workspace-1' };
const entityId = '123e4567-e89b-12d3-a456-426614174010';
const marketContextId = '123e4567-e89b-12d3-a456-426614174002';
const draftKey = (contextId: string) =>
  draftStorageKey({
    editor: draftEditors.entityEdit,
    resource: [entityId, contextId],
    userId: session.user_id,
    workspaceId: session.workspace_id,
  });
const draftProps = (contextId: string) => ({
  contextId,
  draft: {
    editor: draftEditors.entityEdit,
    resource: [entityId, contextId],
    source: '1',
  },
  initialValues: { title: 'Loaded' },
});
const titleBox = () =>
  screen.getByRole('textbox', { hidden: true, name: 'title' });

afterEach(() => {
  sessionStorage.clear();
});

describe('EntityForm', () => {
  const emailBlueprint = (overrides: Partial<Attribute> = {}) => {
    const result = blueprint([attribute('contact', overrides)]);
    result.blueprint.views.edit = {
      type: 'stack',
      children: [
        {
          type: 'field',
          field: 'contact',
          component: { id: 'catalog.email_edit', version: 1, props: {} },
        },
      ],
    };
    return result;
  };

  it('validates configured email input and preserves case and plus tags on save', async () => {
    const { onSubmit } = renderForm({ blueprint: emailBlueprint() });
    const input = screen.getByRole('textbox', { name: 'contact' });
    expect(input.getAttribute('type')).toBe('email');
    fireEvent.change(input, { target: { value: 'not an email' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() =>
      expect(screen.getByText(/single email address/)).toBeTruthy(),
    );
    expect(onSubmit).not.toHaveBeenCalled();
    fireEvent.change(input, { target: { value: 'Name+tag@Example.com' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(onSubmit).toHaveBeenCalledOnce());
    expect(JSON.stringify(onSubmit.mock.calls[0])).toContain(
      'Name+tag@Example.com',
    );
  });

  it('rejects required empty email', async () => {
    const { onSubmit } = renderForm({
      blueprint: emailBlueprint(),
      requiredAttributes: ['contact'],
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(screen.getByRole('textbox')).toHaveProperty('required', true);
    expect(
      await screen.findByText('A value is required for the target schema.'),
    ).toBeTruthy();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it.each([
    { readonly: true },
    { context_editable: 'default' as const },
    { extension_type: { available: false } },
  ])('disables restricted email attributes: %j', async (overrides) => {
    const { onSubmit } = renderForm({
      blueprint: emailBlueprint(overrides as Partial<Attribute>),
      contextId: marketContextId,
    });
    const input = screen.getByRole('textbox', { name: 'contact' });
    expect(input).toHaveProperty('disabled', true);
    fireEvent.change(input, { target: { value: 'changed@example.test' } });
    expect(input).toHaveProperty('value', '');
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it('restores an email draft only on request and validates it', async () => {
    writeDraft(draftKey(marketContextId), {
      savedAt: new Date().toISOString(),
      source: null,
      value: { contact: 'invalid draft' },
    });
    const { onSubmit } = renderForm({
      ...draftProps(marketContextId),
      blueprint: emailBlueprint(),
      initialValues: { contact: 'saved@example.test' },
    });
    await screen.findByRole('dialog', { name: 'Restore unsaved draft?' });
    expect(
      screen.getByRole('textbox', { hidden: true, name: 'contact' }),
    ).toHaveProperty('value', 'saved@example.test');
    fireEvent.click(screen.getByRole('button', { name: 'Restore draft' }));
    expect(
      await screen.findByRole('textbox', { name: 'contact' }),
    ).toHaveProperty('value', 'invalid draft');
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(await screen.findByText(/single email address/)).toBeTruthy();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it('offers a context draft and applies it only when restored', async () => {
    writeDraft(draftKey(marketContextId), {
      savedAt: new Date().toISOString(),
      source: JSON.stringify(['1', JSON.stringify({ title: 'Loaded' })]),
      value: { title: 'Draft title' },
    });
    renderForm({
      blueprint: blueprint([attribute('title')]),
      ...draftProps(marketContextId),
    });

    await screen.findByRole('dialog', { name: 'Restore unsaved draft?' });
    expect(screen.queryByText(/source has changed/)).toBeNull();
    expect(titleBox()).toHaveProperty('value', 'Loaded');
    fireEvent.click(screen.getByRole('button', { name: 'Restore draft' }));
    expect(titleBox()).toHaveProperty('value', 'Draft title');
  });

  it('does not offer a draft from another context', () => {
    writeDraft(draftKey(marketContextId), {
      savedAt: new Date().toISOString(),
      source: null,
      value: { title: 'Draft title' },
    });
    renderForm({
      blueprint: blueprint([attribute('title')]),
      ...draftProps('123e4567-e89b-12d3-a456-426614174001'),
    });
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(titleBox()).toHaveProperty('value', 'Loaded');
  });

  it('warns when the entity changed since the draft was saved', async () => {
    writeDraft(draftKey(marketContextId), {
      savedAt: new Date().toISOString(),
      source: JSON.stringify(['1', JSON.stringify({ title: 'Older' })]),
      value: { title: 'Draft title' },
    });
    renderForm({
      blueprint: blueprint([attribute('title')]),
      ...draftProps(marketContextId),
    });
    expect(await screen.findByText(/source has changed/)).toBeTruthy();
  });

  it('persists edited fields without file or readonly attributes', async () => {
    renderForm({
      blueprint: blueprint([
        attribute('title'),
        attribute('managed', { readonly: true }),
        attribute('photos', { value_type: 'file' }),
      ]),
      ...draftProps(marketContextId),
      initialValues: { managed: 'System', photos: 'file-id', title: 'Loaded' },
    });
    fireEvent.change(titleBox(), { target: { value: 'Edited' } });
    await waitFor(() =>
      expect(
        JSON.parse(sessionStorage.getItem(draftKey(marketContextId)) ?? '{}')
          .value,
      ).toEqual({ title: 'Edited' }),
    );
  });

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
