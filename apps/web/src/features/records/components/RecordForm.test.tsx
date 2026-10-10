// @vitest-environment jsdom
import { createRef } from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '../../../i18n';
import { authQueryKeys } from '../../auth/queryKeys';
import { draftEditors } from '../../drafts/constants';
import { draftStorageKey, writeDraft } from '../../drafts/draftStorage';
import { ApiRequestError } from '../../../api/request';
import type { BlueprintWithAttributes, Attribute } from '../api';
import { RecordForm, type RecordFormHandle } from './RecordForm';
import { recordHeadingComponentId } from '../../views/components/blocks/RecordHeadingDefinition';

const attribute = (
  code: string,
  overrides: Partial<Attribute> = {},
): Attribute => ({
  code,
  value_type: 'string',
  ...overrides,
});

/**
 * A blueprint whose detail view shows one field with a display component; the
 * form edits it with the paired edit component.
 */
const withDisplayComponent = (
  code: string,
  componentId: string,
  overrides: Partial<Attribute> = {},
) => {
  const result = blueprint([attribute(code, overrides)]);
  result.blueprint.views.detail = {
    type: 'stack',
    children: [
      {
        type: 'field',
        field: code,
        component: { id: componentId, version: 1, props: {} },
      },
    ],
  };
  return result;
};

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
  props: Partial<React.ComponentProps<typeof RecordForm>>,
) => {
  const ref = createRef<RecordFormHandle>();
  const onSubmit = vi.fn();
  const client = new QueryClient({
    defaultOptions: { queries: { staleTime: Infinity } },
  });
  client.setQueryData(authQueryKeys.session(), session);
  const tree = (nextProps = props) => (
    <QueryClientProvider client={client}>
      <RecordForm
        blueprint={blueprint([])}
        contextId="123e4567-e89b-12d3-a456-426614174001"
        defaultContextId="123e4567-e89b-12d3-a456-426614174001"
        onSubmit={onSubmit}
        ref={ref}
        submitLabel="Save"
        {...nextProps}
      />
    </QueryClientProvider>
  );
  const view = render(tree());
  return {
    ref,
    onSubmit,
    rerenderForm: (next: typeof props) =>
      view.rerender(tree({ ...props, ...next })),
  };
};

const session = { user_id: 'user-1', workspace_id: 'workspace-1' };
const recordId = '123e4567-e89b-12d3-a456-426614174010';
const marketContextId = '123e4567-e89b-12d3-a456-426614174002';
const draftKey = (contextId: string) =>
  draftStorageKey({
    editor: draftEditors.recordCreate,
    resource: [recordId, contextId],
    userId: session.user_id,
    workspaceId: session.workspace_id,
  });
const draftProps = (contextId: string) => ({
  contextId,
  draft: {
    editor: draftEditors.recordCreate,
    resource: [recordId, contextId],
    source: '1',
  },
  initialValues: { title: 'Loaded' },
});
const titleBox = () =>
  screen.getByRole('textbox', { hidden: true, name: 'title' });

afterEach(() => {
  sessionStorage.clear();
});

describe('RecordForm', () => {
  it.each([false, true])(
    'preserves the values/version baseline across refetches (edited: %s)',
    async (edited) => {
      const originalVersion = '2026-01-01T00:00:00Z';
      const { onSubmit, rerenderForm } = renderForm({
        blueprint: blueprint([attribute('title')]),
        expectedUpdatedAt: originalVersion,
        initialValues: { title: 'Original' },
      });
      if (edited)
        fireEvent.change(titleBox(), { target: { value: 'My edit' } });
      rerenderForm({
        expectedUpdatedAt: '2026-01-02T00:00:00Z',
        initialValues: { title: 'Someone else changed it' },
      });
      expect(titleBox()).toHaveProperty(
        'value',
        edited ? 'My edit' : 'Original',
      );
      fireEvent.click(screen.getByRole('button', { name: 'Save' }));
      await waitFor(() =>
        expect(onSubmit).toHaveBeenCalledWith(
          expect.objectContaining({
            expected_updated_at: originalVersion,
            values: [
              expect.objectContaining({
                value: edited ? 'My edit' : 'Original',
              }),
            ],
          }),
        ),
      );
    },
  );

  it('selects URL editors, blocks invalid programmatic values, saves and clears URLs', async () => {
    const { onSubmit } = renderForm({
      blueprint: withDisplayComponent('website', 'attricat.url_display'),
    });
    const input = screen.getByRole('textbox', { name: 'website' });
    expect(input.getAttribute('type')).toBe('url');
    fireEvent.change(input, { target: { value: 'javascript:alert(1)' } });
    fireEvent.submit(input.closest('form')!);
    await waitFor(() =>
      expect(input.getAttribute('aria-invalid')).toBe('true'),
    );
    expect(onSubmit).not.toHaveBeenCalled();
    fireEvent.change(input, { target: { value: 'https://example.com/a?b=1' } });
    fireEvent.submit(input.closest('form')!);
    await waitFor(() =>
      expect(onSubmit).toHaveBeenCalledWith(
        expect.objectContaining({
          values: [
            expect.objectContaining({ value: 'https://example.com/a?b=1' }),
          ],
        }),
      ),
    );
    onSubmit.mockClear();
    fireEvent.change(input, { target: { value: '' } });
    fireEvent.submit(input.closest('form')!);
    await waitFor(() =>
      expect(onSubmit).toHaveBeenCalledWith(
        expect.objectContaining({ values: [] }),
      ),
    );
  });

  it('dispatches the configured Markdown editor and submits unchanged source', async () => {
    const { onSubmit } = renderForm({
      blueprint: withDisplayComponent(
        'description',
        'attricat.markdown_display',
      ),
    });
    const source = '    code\n\n**Hello**  \nworld\n';
    fireEvent.change(screen.getByRole('textbox', { name: 'description' }), {
      target: { value: source },
    });
    fireEvent.click(screen.getByRole('tab', { name: 'Preview' }));
    expect(screen.getByText('Hello').tagName).toBe('STRONG');
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(onSubmit).toHaveBeenCalled());
    expect(JSON.stringify(onSubmit.mock.calls[0])).toContain(
      JSON.stringify(source),
    );
  });

  const emailBlueprint = (overrides: Partial<Attribute> = {}) =>
    withDisplayComponent('contact', 'attricat.email_display', overrides);

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

  it('disables saving until a required field has a value', () => {
    const { onSubmit } = renderForm({
      blueprint: emailBlueprint(),
      requiredAttributes: ['contact'],
    });
    const save = screen.getByRole('button', { name: 'Save' });
    expect(screen.getByRole('textbox')).toHaveProperty('required', true);
    expect(save).toHaveProperty('disabled', true);
    fireEvent.click(save);
    expect(onSubmit).not.toHaveBeenCalled();
    fireEvent.change(screen.getByRole('textbox'), {
      target: { value: 'name@example.com' },
    });
    expect(save).toHaveProperty('disabled', false);
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

  const colorBlueprint = (overrides: Partial<Attribute> = {}) => {
    const result = blueprint([attribute('hex', overrides)]);
    result.blueprint.views.detail = {
      type: 'tabs',
      tabs: [
        {
          label: 'Color',
          children: [
            {
              type: 'section',
              children: [
                {
                  type: 'field',
                  field: 'hex',
                  component: {
                    id: 'attricat.color_edit',
                    version: 1,
                    props: {},
                  },
                },
              ],
            },
          ],
        },
      ],
    };
    return result;
  };

  it.each([false, true])(
    'validates and saves configured colors with showAll=%s',
    async (showAllAttributes) => {
      const { onSubmit } = renderForm({
        blueprint: colorBlueprint(),
        showAllAttributes,
      });
      const text = screen.getByRole('textbox', { name: 'hex' });
      fireEvent.change(text, { target: { value: '#fff' } });
      fireEvent.click(screen.getByRole('button', { name: 'Save' }));
      await waitFor(() =>
        expect(screen.getByText(/Enter a six-digit/)).toBeTruthy(),
      );
      expect(onSubmit).not.toHaveBeenCalled();
      fireEvent.change(text, { target: { value: '#aBcDeF' } });
      fireEvent.click(screen.getByRole('button', { name: 'Save' }));
      await waitFor(() =>
        expect(onSubmit).toHaveBeenCalledWith(
          expect.objectContaining({
            values: [
              expect.objectContaining({
                attribute_code: 'hex',
                value: '#aBcDeF',
              }),
            ],
          }),
        ),
      );
    },
  );

  it.each([{ readonly: true }, { context_editable: 'default' as const }])(
    'does not mutate restricted colors: %j',
    async (overrides) => {
      const { onSubmit } = renderForm({
        blueprint: colorBlueprint(overrides),
        contextId: marketContextId,
      });
      const input = screen.getByRole('textbox', { name: 'hex' });
      expect(input).toHaveProperty('disabled', true);
      expect(screen.getByLabelText('Pick color for hex')).toHaveProperty(
        'disabled',
        true,
      );
      fireEvent.change(input, { target: { value: '#ffffff' } });
      fireEvent.click(screen.getByRole('button', { name: 'Save' }));
      await waitFor(() =>
        expect(onSubmit).toHaveBeenCalledWith(
          expect.objectContaining({ values: [] }),
        ),
      );
    },
  );

  it('rejects clearing required colors', async () => {
    const { onSubmit } = renderForm({
      blueprint: colorBlueprint(),
      requiredAttributes: ['hex'],
      initialValues: { hex: '#ffffff' },
    });
    fireEvent.change(screen.getByRole('textbox', { name: 'hex' }), {
      target: { value: '' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() =>
      expect(
        screen
          .getByRole('textbox', { name: 'hex' })
          .getAttribute('aria-invalid'),
      ).toBe('true'),
    );
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it('restores color drafts', async () => {
    writeDraft(draftKey(marketContextId), {
      savedAt: new Date().toISOString(),
      source: JSON.stringify(['1', JSON.stringify({ hex: '' })]),
      value: { hex: '#aBcDeF' },
    });
    renderForm({
      blueprint: colorBlueprint(),
      ...draftProps(marketContextId),
      initialValues: { hex: '' },
    });
    await screen.findByRole('dialog', { name: 'Restore unsaved draft?' });
    expect(
      screen.getByRole('textbox', { name: 'hex', hidden: true }),
    ).toHaveProperty('value', '');
    fireEvent.click(screen.getByRole('button', { name: 'Restore draft' }));
    expect(await screen.findByRole('textbox', { name: 'hex' })).toHaveProperty(
      'value',
      '#aBcDeF',
    );
    expect(screen.getByLabelText('Pick color for hex')).toHaveProperty(
      'value',
      '#abcdef',
    );
  });

  it('does not interpret ordinary strings as configured colors', async () => {
    const { onSubmit } = renderForm({
      blueprint: blueprint([attribute('hex')]),
      initialValues: { hex: 'red' },
    });
    expect(screen.queryByLabelText('Pick color for hex')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() =>
      expect(onSubmit).toHaveBeenCalledWith(
        expect.objectContaining({
          values: [expect.objectContaining({ value: 'red' })],
        }),
      ),
    );
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

  it('warns when the record changed since the draft was saved', async () => {
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

  it('renders editable attributes the detail view omits so they can be saved', async () => {
    const result = blueprint([
      attribute('title'),
      attribute('sku'),
      attribute('notes'),
      attribute('internal', { tags: ['hidden:form'] }),
    ]);
    result.blueprint.record_schema = {
      type: 'object',
      required: ['title', 'sku'],
    };
    result.blueprint.views.detail = {
      type: 'stack',
      children: [
        {
          type: 'stack',
          // The heading only displays values, so its fields are edited first.
          component: { id: recordHeadingComponentId, version: 1, props: {} },
          children: [{ type: 'field', field: 'title' }],
        },
        { type: 'field', field: 'notes' },
      ],
    };
    const { onSubmit } = renderForm({ blueprint: result });
    expect(screen.getByText('Other attributes')).toBeTruthy();
    const notes = screen.getByRole('textbox', { name: 'notes' });
    expect(
      titleBox().compareDocumentPosition(notes) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(screen.queryByRole('textbox', { name: 'internal' })).toBeNull();
    fireEvent.change(titleBox(), { target: { value: 'Shirt' } });
    fireEvent.change(screen.getByRole('textbox', { name: 'sku' }), {
      target: { value: 'SKU-1' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(onSubmit).toHaveBeenCalledOnce());
  });

  it('names a cleared schema-required field as required', () => {
    const result = blueprint([attribute('title')]);
    result.blueprint.record_schema = { type: 'object', required: ['title'] };
    renderForm({ blueprint: result });
    const save = screen.getByRole('button', { name: 'Save' });
    expect(titleBox()).toHaveProperty('required', true);
    expect(save).toHaveProperty('disabled', true);

    fireEvent.change(titleBox(), { target: { value: 'Shirt' } });
    expect(save).toHaveProperty('disabled', false);
    fireEvent.change(titleBox(), { target: { value: '   ' } });

    expect(screen.getByText('A value is required.')).toBeTruthy();
    expect(save).toHaveProperty('disabled', true);
  });

  it('offers a required field hidden from forms once when the detail view has no layout', () => {
    const result = blueprint([
      attribute('title'),
      attribute('code', { tags: ['hidden:form'] }),
    ]);
    result.blueprint.record_schema = { type: 'object', required: ['code'] };
    result.blueprint.views.detail = { type: 'table', columns: [] } as never;
    renderForm({ blueprint: result });

    expect(screen.getAllByRole('textbox', { name: 'code' })).toHaveLength(1);
    expect(screen.queryByText('Other attributes')).toBeNull();
  });

  it('offers a required field hidden from forms when the blueprint has no detail view', () => {
    const result = blueprint([
      attribute('title'),
      attribute('code', { tags: ['hidden:form'] }),
      attribute('notes', { tags: ['hidden:form'] }),
    ]);
    result.blueprint.record_schema = { type: 'object', required: ['code'] };
    delete result.blueprint.views.detail;
    renderForm({ blueprint: result });

    expect(screen.getAllByRole('textbox', { name: 'code' })).toHaveLength(1);
    expect(screen.getByText('Other attributes')).toBeTruthy();
    expect(screen.queryByRole('textbox', { name: 'notes' })).toBeNull();
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

  it('shows declarative check violations on their fields and summarizes the rest', () => {
    const error = new ApiRequestError(
      422,
      'Record checks failed',
      'record_check_failed',
      {
        violations: [
          {
            source: 'record_check',
            code: 'valid-range',
            message: 'Valid until must not be before valid from',
            contexts: ['default'],
            attributes: ['valid_until'],
            evidence: {},
          },
          {
            source: 'record_check',
            code: 'facility-of-supplier',
            message: 'Facility must belong to the supplier',
            contexts: ['default'],
            attributes: [],
            evidence: {},
          },
        ],
      },
    );
    renderForm({
      blueprint: blueprint([attribute('title'), attribute('valid_until')]),
      error,
    });
    expect(
      screen.getByText(
        'Valid until must not be before valid from (contexts: default)',
      ),
    ).toBeTruthy();
    expect(screen.getByText('Record checks failed')).toBeTruthy();
    expect(
      screen.getByText(
        'Facility must belong to the supplier (contexts: default)',
      ),
    ).toBeTruthy();

    fireEvent.change(
      screen.getByRole('textbox', { hidden: true, name: 'valid until' }),
      {
        target: { value: 'fixed' },
      },
    );
    expect(
      screen.queryByText(
        'Valid until must not be before valid from (contexts: default)',
      ),
    ).toBeNull();
  });
});
