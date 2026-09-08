// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { listPermissions, listRoles } from './api';
import { WorkspaceRolesSection } from './WorkspaceRolesSection';

vi.mock('./api', () => ({
  createRole: vi.fn(),
  duplicateRole: vi.fn(),
  listPermissions: vi.fn(),
  listRoles: vi.fn(),
  retireRole: vi.fn(),
  updateRole: vi.fn(),
}));

const renderRoles = () => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });

  return render(
    <QueryClientProvider client={client}>
      <WorkspaceRolesSection canManage />
    </QueryClientProvider>,
  );
};

beforeEach(() => {
  vi.mocked(listPermissions).mockResolvedValue([]);
  vi.mocked(listRoles).mockResolvedValue([
    {
      id: '123e4567-e89b-12d3-a456-426614174000',
      code: 'editor',
      created_at: '2026-01-01T00:00:00Z',
      is_system: false,
      permissions: [],
    },
  ]);
});

describe('WorkspaceRolesSection', () => {
  it.each([
    ['Rename role', 'Role code'],
    ['Duplicate role', 'New role code'],
    ['Retire role', 'Replacement role ID'],
  ])('focuses the %s dialog field when it opens', async (action, label) => {
    renderRoles();

    fireEvent.click(await screen.findByRole('button', { name: action }));
    const input = await screen.findByRole('textbox', { name: label });

    await waitFor(() => expect(document.activeElement).toBe(input));
  });
});
