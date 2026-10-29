// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import type { Attribute } from '../records/api';
import { RecordView } from '../views/components/RecordView';
import { useActionDialogStore } from './actionDialogStore';
import type { ExtensionContribution } from './api';

const { request } = vi.hoisted(() => ({ request: vi.fn() }));
vi.mock('../../api/request', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../api/request')>()),
  request,
}));
vi.mock('./constants', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./constants')>()),
  fieldNotesRetryInterval: 10,
}));
vi.mock('./useExtensionRuntime', () => ({
  useExtensionRuntime: () => ({ data: [explain] }),
}));

import { FieldNotesProvider } from './fieldNotes';

const recordId = '33333333-3333-4333-8333-333333333333';
const blueprintId = '22222222-2222-4222-8222-222222222222';
const fileId = '55555555-5555-4555-8555-555555555555';
const explain: ExtensionContribution = {
  contribution_key: 'acme.tools:explain',
  display_order: 0,
  navigation_group: null,
  capabilities: [],
  configuration: null,
  extension_id: 'acme.tools',
  extension_name: 'Acme tools',
  id: 'explain',
  kind: 'action',
  outlet: 'record_attribute_action',
  release_id: '11111111-1111-4111-8111-111111111111',
  route: null,
  title: 'Explain',
  version: 1,
  invoke: { dialog: 'explain-dialog' },
};
const attributes = [
  { id: 'a1', code: 'price', name: 'Price', value_type: 'number' },
  { id: 'a2', code: 'sku', name: 'SKU', value_type: 'string' },
  { id: 'a3', code: 'attachments', name: 'Attachments', value_type: 'file' },
] as unknown as Attribute[];

const notes = (incomplete = false) => ({
  extensions: [
    {
      extension_id: 'acme.tools',
      extension_name: 'Acme tools',
      attributes: {
        price: {
          badge: { label: 'Computed', tone: 'info', action: 'explain' },
          note: 'price = net * 1.23',
        },
        sku: { badge: { label: 'Checked', tone: 'success', action: 'gone' } },
      },
      files: { [fileId]: { note: 'Last document: summary' } },
    },
  ],
  incomplete,
});

const renderRecord = () =>
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <FieldNotesProvider
        contextId={null}
        recordId={recordId}
        runtimeScope={{ blueprintId, blueprintVersion: 2 }}
      >
        <RecordView
          attributes={attributes}
          values={{
            price: { value: 123 },
            sku: { value: 'L-1' },
            attachments: { value: [{ id: fileId, filename: 'report.pdf' }] },
          }}
        />
      </FieldNotesProvider>
    </QueryClientProvider>,
  );

beforeEach(() => {
  request.mockReset();
  useActionDialogStore.getState().close();
});

describe('field notes', () => {
  it("shows each field's badges and notes below it, and file notes below the file", async () => {
    request.mockResolvedValue(notes());
    renderRecord();

    expect(await screen.findByText('price = net * 1.23')).toBeTruthy();
    expect(screen.getByText('Computed')).toBeTruthy();
    expect(screen.getByText('Last document: summary')).toBeTruthy();
    expect(request).toHaveBeenCalledWith(
      `/api/records/${recordId}/extension-field-notes`,
      expect.anything(),
    );
    // The badge's tooltip names the extension it comes from.
    expect(
      screen.getByText('Computed').closest('[title="Acme tools"]'),
    ).toBeTruthy();
  });

  it("runs a badge's action with the field in its context", async () => {
    request.mockResolvedValue(notes());
    renderRecord();

    await userEvent.click(
      await screen.findByRole('button', { name: 'Computed' }),
    );
    expect(useActionDialogStore.getState().dialog).toMatchObject({
      extensionId: 'acme.tools',
      dialogId: 'explain-dialog',
      context: {
        action_id: 'explain',
        outlet: 'record_attribute_action',
        record_ids: [recordId],
        attribute_code: 'price',
      },
    });
    // A badge naming no enabled action is not a button.
    expect(screen.queryByRole('button', { name: 'Checked' })).toBeNull();
    expect(screen.getByText('Checked')).toBeTruthy();
  });

  it('asks again while a provider is still starting', async () => {
    request.mockResolvedValueOnce(notes(true)).mockResolvedValue(notes());
    renderRecord();

    await waitFor(() => expect(request).toHaveBeenCalledTimes(2));
    expect(await screen.findByText('price = net * 1.23')).toBeTruthy();
  });
});
