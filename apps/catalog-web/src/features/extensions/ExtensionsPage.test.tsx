// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { currentSession } from '../auth/api';
import { ExtensionsPage } from './ExtensionsPage';
import {
  discoverExtensions,
  installedExtensions,
  updateWorkspaceExtensionLayout,
  workspaceExtensionLayout,
} from './managementApi';

vi.mock('@tanstack/react-router', () => ({
  createLink: <T,>(component: T) => component,
  Link: ({ children }: { children: React.ReactNode }) => (
    <span>{children}</span>
  ),
}));

vi.mock('../auth/api', () => ({
  currentSession: vi.fn().mockResolvedValue({
    capabilities: { extensions_manage: true, extensions_read: true },
  }),
}));

vi.mock('./managementApi', () => ({
  discoverExtensions: vi.fn().mockResolvedValue([]),
  installedExtensions: vi.fn().mockResolvedValue([]),
  updateWorkspaceExtensionLayout: vi.fn().mockResolvedValue(undefined),
  workspaceExtensionLayout: vi.fn(),
}));

const renderPage = () => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <ExtensionsPage />
    </QueryClientProvider>,
  );
};

describe('ExtensionsPage layout form', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(currentSession).mockResolvedValue({
      capabilities: { extensions_manage: true, extensions_read: true },
    } as never);
    vi.mocked(discoverExtensions).mockResolvedValue([]);
    vi.mocked(installedExtensions).mockResolvedValue([]);
    vi.mocked(updateWorkspaceExtensionLayout).mockResolvedValue(undefined);
    vi.mocked(workspaceExtensionLayout).mockResolvedValue({
      version: 1,
      outlets: {},
    });
  });

  it('shows validation feedback and submits a strict versioned layout', async () => {
    const user = userEvent.setup();
    renderPage();
    const editor = await screen.findByRole('textbox', {
      name: 'Extension layout',
    });

    await user.click(editor);
    await user.clear(editor);
    await user.paste('{"version":1,"outlets":{"unknown":{}}}');
    await user.tab();
    expect(
      await screen.findByText(
        'Enter a valid version 1 extension layout with known outlets and unique contribution keys.',
      ),
    ).toBeTruthy();

    await user.click(editor);
    await user.clear(editor);
    await user.paste(
      JSON.stringify({
        version: 1,
        outlets: {
          navigation: { order: [], hidden: [], promoted: [] },
        },
      }),
    );
    await user.tab();
    const save = screen.getByRole('button', { name: 'Save layout' });
    expect(save.hasAttribute('disabled')).toBe(false);
    await user.click(save);
    expect(
      vi.mocked(updateWorkspaceExtensionLayout).mock.calls[0]?.[0],
    ).toEqual({
      version: 1,
      outlets: {
        navigation: { order: [], hidden: [], promoted: [] },
      },
    });
  });
});
