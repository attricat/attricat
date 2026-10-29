// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { useActionDialogStore } from './actionDialogStore';
import { usePaletteSurface } from './actionPaletteStore';
import type { ExtensionContribution } from './api';

vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  getExtensionRuntime: vi.fn(),
}));
vi.mock('@tanstack/react-router', () => ({ useNavigate: () => vi.fn() }));

import { getExtensionRuntime } from './api';
import { ActionPalette } from './ActionPalette';

const blueprintId = '22222222-2222-4222-8222-222222222222';
const recordId = '33333333-3333-4333-8333-333333333333';
const action = (id: string, title: string): ExtensionContribution => ({
  contribution_key: `acme.tools:${id}`,
  display_order: 0,
  navigation_group: null,
  capabilities: [],
  configuration: null,
  extension_id: 'acme.tools',
  extension_name: 'Acme tools',
  id,
  kind: 'action',
  outlet: 'explorer_bulk_action',
  release_id: '11111111-1111-4111-8111-111111111111',
  route: null,
  title,
  version: 1,
  invoke: { dialog: 'documents' },
  when: null,
  confirm: null,
  icon: 'file',
  translations: {},
});

const Page = () => {
  usePaletteSurface('selection', {
    outlet: 'explorer_bulk_action',
    runtimeScope: { blueprintId, blueprintVersion: 1 },
    target: {
      blueprintId,
      blueprintVersion: 1,
      contextId: null,
      recordIds: [recordId],
    },
  });
  return null;
};

beforeEach(() => useActionDialogStore.getState().close());

describe('ActionPalette', () => {
  it("opens with Ctrl+K, searches the page's actions and runs the chosen one", async () => {
    vi.mocked(getExtensionRuntime).mockResolvedValue([
      action('documents', 'Generate documents'),
      action('recalculate', 'Recalculate formulas'),
    ]);
    render(
      <QueryClientProvider client={new QueryClient()}>
        <Page />
        <ActionPalette />
      </QueryClientProvider>,
    );
    const user = userEvent.setup();
    await user.keyboard('{Control>}k{/Control}');
    await user.type(await screen.findByLabelText('Search actions'), 'recal');

    expect(
      screen.queryByRole('button', { name: /Generate documents/ }),
    ).toBeNull();
    await user.click(
      await screen.findByRole('button', { name: /Recalculate formulas/ }),
    );
    expect(useActionDialogStore.getState().dialog).toMatchObject({
      dialogId: 'documents',
      context: {
        action_id: 'recalculate',
        outlet: 'explorer_bulk_action',
        record_ids: [recordId],
      },
    });
  });
});
