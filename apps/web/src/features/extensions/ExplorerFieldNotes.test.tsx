// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';

const { request } = vi.hoisted(() => ({ request: vi.fn() }));
vi.mock('../../api/request', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../api/request')>()),
  request,
}));
vi.mock('./useExtensionRuntime', () => ({
  useExtensionRuntime: () => ({ data: [] }),
}));
vi.mock('@tanstack/react-router', () => ({ useNavigate: () => vi.fn() }));
vi.mock('../auth/useCan', () => ({
  useCan: () => true,
  useAccess: () => ({ can: () => true, isPending: false }),
}));

import {
  ExplorerCellNotes,
  ExplorerFieldNotesProvider,
} from './ExplorerFieldNotes';

const first = '33333333-3333-4333-8333-333333333333';
const second = '44444444-4444-4444-8444-444444444444';

describe('Explorer field notes', () => {
  it("asks once for the loaded rows and badges each row's cell", async () => {
    request.mockResolvedValue({
      records: {
        [first]: [
          {
            extension_id: 'acme.tools',
            extension_name: 'Acme tools',
            attributes: {
              price: {
                badge: { label: 'Computed', tone: 'info' },
                note: 'price = net * 1.23',
              },
            },
            files: {},
            actions: {},
          },
        ],
        [second]: [],
      },
      incomplete: false,
      client_sources: [],
    });
    render(
      <QueryClientProvider client={new QueryClient()}>
        <ExplorerFieldNotesProvider
          blueprintCode="product"
          contextId={null}
          recordIds={[first, second]}
          runtimeScope={{
            blueprintId: '22222222-2222-4222-8222-222222222222',
            blueprintVersion: 1,
          }}
        >
          {[first, second].map((recordId) => (
            <ExplorerCellNotes
              attributeCode="price"
              key={recordId}
              recordId={recordId}
              recordTags={[]}
            >
              <span>value {recordId.slice(0, 2)}</span>
            </ExplorerCellNotes>
          ))}
        </ExplorerFieldNotesProvider>
      </QueryClientProvider>,
    );

    expect(await screen.findByText('Computed')).toBeTruthy();
    expect(screen.getAllByText('Computed')).toHaveLength(1);
    expect(screen.getByText('value 44')).toBeTruthy();
    // A cell has no room for the note, so the badge's tooltip carries it.
    expect(
      screen
        .getByText('Computed')
        .closest('[title="price = net * 1.23 · Acme tools"]'),
    ).toBeTruthy();
    expect(request).toHaveBeenCalledTimes(1);
    expect(JSON.parse(request.mock.calls[0][2].body).record_ids).toEqual([
      first,
      second,
    ]);
  });
});
