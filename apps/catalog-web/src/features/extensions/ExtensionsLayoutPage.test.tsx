// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { authQueryKeys } from '../auth/query-keys';
import { currentSession } from '../auth/api';
import { workspaceExtensionLayout } from './management-api';
import { extensionManagementQueryKeys } from './management-query-keys';
import { ExtensionsLayoutPage } from './ExtensionsLayoutPage';

vi.mock('../auth/api', () => ({ currentSession: vi.fn() }));
vi.mock('./management-api', () => ({
  workspaceExtensionLayout: vi.fn(),
  updateWorkspaceExtensionLayout: vi.fn(),
}));

const layout = (order: string[]) => ({
  version: 1 as const,
  outlets: { navigation: { order, hidden: [] } },
});

describe('ExtensionsLayoutPage', () => {
  it('hydrates a pristine form from a refreshed server layout', async () => {
    const client = new QueryClient();
    vi.mocked(currentSession).mockResolvedValue({
      capabilities: { extensions_manage: true },
    } as never);
    vi.mocked(workspaceExtensionLayout).mockResolvedValue(layout([]) as never);
    client.setQueryData(authQueryKeys.session(), {
      capabilities: { extensions_manage: true },
    });
    client.setQueryData(extensionManagementQueryKeys.layout(), layout([]));
    render(
      <QueryClientProvider client={client}>
        <ExtensionsLayoutPage />
      </QueryClientProvider>,
    );
    const field = screen.getByRole('textbox', { name: 'Extension layout' });
    await waitFor(() =>
      expect(field).toHaveProperty(
        'value',
        JSON.stringify(layout([]), null, 2),
      ),
    );
    client.setQueryData(
      extensionManagementQueryKeys.layout(),
      layout(['server']),
    );
    await waitFor(() =>
      expect(field).toHaveProperty(
        'value',
        JSON.stringify(layout(['server']), null, 2),
      ),
    );
  });

  it('keeps an unsaved draft when the layout refetches in the background', async () => {
    const client = new QueryClient();
    vi.mocked(currentSession).mockResolvedValue({
      capabilities: { extensions_manage: true },
    } as never);
    vi.mocked(workspaceExtensionLayout).mockResolvedValue(layout([]) as never);
    client.setQueryData(authQueryKeys.session(), {
      capabilities: { extensions_manage: true },
    });
    client.setQueryData(extensionManagementQueryKeys.layout(), layout([]));
    render(
      <QueryClientProvider client={client}>
        <ExtensionsLayoutPage />
      </QueryClientProvider>,
    );
    const field = screen.getByRole('textbox', { name: 'Extension layout' });
    await waitFor(() =>
      expect(field).toHaveProperty(
        'value',
        JSON.stringify(layout([]), null, 2),
      ),
    );
    const draft = JSON.stringify(layout(['custom']), null, 2);
    fireEvent.change(field, { target: { value: draft } });
    client.setQueryData(
      extensionManagementQueryKeys.layout(),
      layout(['server']),
    );
    await waitFor(() => expect(field).toHaveProperty('value', draft));
  });
});
