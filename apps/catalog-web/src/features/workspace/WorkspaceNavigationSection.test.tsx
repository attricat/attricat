// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { listEntityBlueprints } from '../entities/api';
import {
  listExploreNavigation,
  listRoles,
  updateExploreNavigation,
} from './api';
import { WorkspaceNavigationSection } from './WorkspaceNavigationSection';

vi.mock('../entities/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../entities/api')>()),
  listEntityBlueprints: vi.fn(),
}));
vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  listExploreNavigation: vi.fn(),
  listRoles: vi.fn(),
  updateExploreNavigation: vi.fn(),
}));

describe('WorkspaceNavigationSection', () => {
  it('allows typing multiple comma-separated roles without eating the delimiter', async () => {
    vi.mocked(listExploreNavigation).mockResolvedValue([
      { blueprint_code: 'product', visible_to_role_codes: [] },
    ]);
    vi.mocked(listEntityBlueprints).mockResolvedValue([
      { code: 'product', name: 'Product' },
    ] as never);
    vi.mocked(listRoles).mockResolvedValue([]);
    render(
      <QueryClientProvider client={new QueryClient()}>
        <WorkspaceNavigationSection canManage />
      </QueryClientProvider>,
    );
    const roles = await screen.findByRole('textbox', {
      name: 'Visible roles (comma separated)',
    });
    fireEvent.change(roles, { target: { value: 'reader,' } });
    expect(roles).toHaveProperty('value', 'reader,');
    fireEvent.change(roles, { target: { value: 'reader, editor' } });
    expect(roles).toHaveProperty('value', 'reader, editor');
  });

  it('prevents assigning the same blueprint to two shortcuts', async () => {
    vi.mocked(listExploreNavigation).mockResolvedValue([
      { blueprint_code: 'product', visible_to_role_codes: [] },
      { blueprint_code: 'other', visible_to_role_codes: [] },
    ]);
    vi.mocked(listEntityBlueprints).mockResolvedValue([
      { code: 'product', name: 'Product' },
      { code: 'other', name: 'Other' },
    ] as never);
    vi.mocked(listRoles).mockResolvedValue([]);
    render(
      <QueryClientProvider client={new QueryClient()}>
        <WorkspaceNavigationSection canManage />
      </QueryClientProvider>,
    );
    const selects = await screen.findAllByRole('combobox', {
      name: 'Blueprint',
    });
    fireEvent.mouseDown(selects[1]);
    expect(
      (
        await screen.findByRole('option', { name: 'Product (product)' })
      ).getAttribute('aria-disabled'),
    ).toBe('true');
  });

  it('keeps the saved navigation snapshot stable until the request finishes', async () => {
    vi.mocked(listExploreNavigation).mockResolvedValue([
      { blueprint_code: 'product', visible_to_role_codes: [] },
    ]);
    vi.mocked(listEntityBlueprints).mockResolvedValue([
      { code: 'product', name: 'Product' },
      { code: 'other', name: 'Other' },
    ] as never);
    vi.mocked(listRoles).mockResolvedValue([]);
    vi.mocked(updateExploreNavigation).mockImplementation(
      () => new Promise(() => {}),
    );
    render(
      <QueryClientProvider client={new QueryClient()}>
        <WorkspaceNavigationSection canManage />
      </QueryClientProvider>,
    );
    const save = await screen.findByRole('button', { name: 'Save navigation' });
    await screen.findByRole('button', { name: 'Remove' });
    fireEvent.click(save);
    await waitFor(() => expect(updateExploreNavigation).toHaveBeenCalledOnce());
    expect(
      screen.getByRole('button', { name: 'Remove' }).hasAttribute('disabled'),
    ).toBe(true);
    expect(
      screen
        .getByRole('button', { name: 'Add shortcut' })
        .hasAttribute('disabled'),
    ).toBe(true);
    expect(
      screen
        .getByRole('textbox', { name: 'Visible roles (comma separated)' })
        .hasAttribute('disabled'),
    ).toBe(true);
  });
});
