// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { listRecordBlueprints } from '../records/api';
import {
  listExploreNavigation,
  listRoles,
  updateExploreNavigation,
} from './api';
import { WorkspaceNavigationSection } from './WorkspaceNavigationSection';

vi.mock('../records/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../records/api')>()),
  listRecordBlueprints: vi.fn(),
}));
vi.mock('./api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./api')>()),
  listExploreNavigation: vi.fn(),
  listRoles: vi.fn(),
  updateExploreNavigation: vi.fn(),
}));

describe('WorkspaceNavigationSection', () => {
  it('picks visible roles from the workspace roles and keeps saved ones', async () => {
    vi.mocked(listExploreNavigation).mockResolvedValue([
      { blueprint_code: 'product', visible_to_role_codes: ['retired'] },
    ]);
    vi.mocked(listRecordBlueprints).mockResolvedValue([
      { code: 'product', name: 'Product' },
    ] as never);
    vi.mocked(listRoles).mockResolvedValue([
      { code: 'reader' },
      { code: 'editor' },
    ] as never);
    render(
      <QueryClientProvider client={new QueryClient()}>
        <WorkspaceNavigationSection canManage />
      </QueryClientProvider>,
    );
    const roles = await screen.findByRole('combobox', {
      name: 'Visible to roles',
    });
    expect(roles.textContent).toBe('retired');
    fireEvent.mouseDown(roles);
    fireEvent.click(await screen.findByRole('option', { name: 'reader' }));
    fireEvent.click(screen.getByRole('option', { name: 'editor' }));
    expect(roles.textContent).toBe('retired, reader, editor');
  });

  it('prevents assigning the same blueprint to two shortcuts', async () => {
    vi.mocked(listExploreNavigation).mockResolvedValue([
      { blueprint_code: 'product', visible_to_role_codes: [] },
      { blueprint_code: 'other', visible_to_role_codes: [] },
    ]);
    vi.mocked(listRecordBlueprints).mockResolvedValue([
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
    vi.mocked(listRecordBlueprints).mockResolvedValue([
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
        .getByRole('combobox', { name: 'Visible to roles' })
        .getAttribute('aria-disabled'),
    ).toBe('true');
  });

  it('keeps chosen roles and saves plain entries after changing a blueprint', async () => {
    vi.mocked(listExploreNavigation).mockResolvedValue([
      { blueprint_code: 'product', visible_to_role_codes: [] },
    ]);
    vi.mocked(listRecordBlueprints).mockResolvedValue([
      { code: 'product', name: 'Product' },
      { code: 'other', name: 'Other' },
    ] as never);
    vi.mocked(listRoles).mockResolvedValue([{ code: 'reader' }] as never);
    vi.mocked(updateExploreNavigation).mockResolvedValue(undefined as never);
    render(
      <QueryClientProvider client={new QueryClient()}>
        <WorkspaceNavigationSection canManage />
      </QueryClientProvider>,
    );
    const roles = await screen.findByRole('combobox', {
      name: 'Visible to roles',
    });
    fireEvent.mouseDown(roles);
    fireEvent.click(await screen.findByRole('option', { name: 'reader' }));
    fireEvent.keyDown(screen.getByRole('listbox'), { key: 'Escape' });
    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Blueprint' }));
    fireEvent.click(
      await screen.findByRole('option', { name: 'Other (other)' }),
    );
    expect(roles.isConnected).toBe(true);
    expect(roles.textContent).toBe('reader');
    expect(
      screen
        .getByRole('button', { name: 'Add shortcut' })
        .hasAttribute('disabled'),
    ).toBe(false);

    fireEvent.click(screen.getByRole('button', { name: 'Add shortcut' }));
    expect(
      screen
        .getByRole('button', { name: 'Add shortcut' })
        .hasAttribute('disabled'),
    ).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: 'Save navigation' }));
    await waitFor(() =>
      expect(vi.mocked(updateExploreNavigation).mock.calls[0]?.[0]).toEqual([
        { blueprint_code: 'other', visible_to_role_codes: ['reader'] },
        { blueprint_code: 'product', visible_to_role_codes: [] },
      ]),
    );
  });
});
