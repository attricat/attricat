// @vitest-environment jsdom
import { cleanup, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import type { RecordItem } from '../records/api';
import { RecordActionsMenu } from './RecordActionsMenu';

const offered = vi.hoisted(() => ({ actions: [] as unknown[] }));

vi.mock('../auth/useCan', () => ({
  useAccess: () => ({ isPending: false, can: () => false }),
}));
vi.mock('../extensions/explorerFieldNotesContext', () => ({
  useExplorerActionStates: () => undefined,
}));
vi.mock('../extensions/ExtensionActions', () => ({
  ExtensionActionMenuItems: () => null,
}));
vi.mock('../extensions/useOfferedActions', () => ({
  useOfferedActions: () => offered.actions,
}));

const renderMenu = (canDelete = false) =>
  render(
    <RecordActionsMenu
      blueprintId="blueprint"
      canDelete={canDelete}
      canDuplicate={false}
      canPublish={false}
      duplicate={vi.fn()}
      onClose={vi.fn()}
      onDelete={vi.fn()}
      position={{ left: 0, top: 0 }}
      publication={undefined}
      publicationContextId={undefined}
      publish={vi.fn()}
      publishing={false}
      record={{ id: 'record', system_tags: [] } as unknown as RecordItem}
      unpublish={vi.fn()}
      unpublishing={false}
    />,
  );

describe('RecordActionsMenu', () => {
  beforeEach(() => {
    cleanup();
    offered.actions = [];
  });

  it('says so when a record has no actions', () => {
    renderMenu();
    const note = screen.getByRole('menuitem', { name: 'No actions available' });
    expect(note.getAttribute('aria-disabled')).toBe('true');
  });

  it('leaves the note out when any action is offered', () => {
    renderMenu(true);
    expect(
      screen.queryByRole('menuitem', { name: 'No actions available' }),
    ).toBeNull();
    expect(
      screen.getByRole('menuitem', { name: 'Delete record' }),
    ).toBeTruthy();
  });

  it('counts actions extensions offer', () => {
    offered.actions = [{}];
    renderMenu();
    expect(
      screen.queryByRole('menuitem', { name: 'No actions available' }),
    ).toBeNull();
  });
});
