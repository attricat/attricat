// @vitest-environment jsdom
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import type { ExplorerColumnPreferences } from './columnPreferences';
import { ExplorerColumnPreferencesDialog } from './ExplorerColumnPreferencesDialog';

const columns = [
  { id: 'title', label: 'Title' },
  { id: 'stock', label: 'Stock' },
  { id: 'price', label: 'Price' },
];

const renderDialog = (preferences: ExplorerColumnPreferences) => {
  const onChange = vi.fn();
  render(
    <ExplorerColumnPreferencesDialog
      columns={columns}
      onChange={onChange}
      onClear={vi.fn()}
      onClose={vi.fn()}
      open
      preferences={preferences}
    />,
  );
  return onChange;
};

describe('ExplorerColumnPreferencesDialog', () => {
  it('offers a labelled drag handle with keyboard instructions per column', async () => {
    renderDialog({ hidden: [], order: ['title', 'stock', 'price'] });
    const list = screen.getByRole('list', { name: 'Columns' });
    const handles = within(list).getAllByRole('button', {
      name: /^Drag to reorder/,
    });
    expect(handles.map((handle) => handle.getAttribute('aria-label'))).toEqual([
      'Drag to reorder Title',
      'Drag to reorder Stock',
      'Drag to reorder Price',
    ]);
    // dnd-kit's accessibility plugin adds the instructions after mounting.
    await waitFor(() =>
      expect(
        document.getElementById(
          handles[0].getAttribute('aria-describedby') ?? '',
        )?.textContent,
      ).toMatch(/press space or enter/i),
    );
  });

  it('keeps the arrow buttons as a non-drag way to reorder', async () => {
    const user = userEvent.setup();
    const onChange = renderDialog({
      hidden: ['price'],
      order: ['title', 'stock', 'price'],
    });
    const [firstUp] = screen.getAllByRole('button', { name: 'Move up' });
    const [firstDown] = screen.getAllByRole('button', { name: 'Move down' });
    expect(firstUp).toHaveProperty('disabled', true);

    await user.click(firstDown);
    expect(onChange).toHaveBeenLastCalledWith({
      hidden: ['price'],
      order: ['stock', 'title', 'price'],
    });

    await user.click(
      screen.getByRole('checkbox', { name: 'Show Price column' }),
    );
    expect(onChange).toHaveBeenLastCalledWith({
      hidden: [],
      order: ['title', 'stock', 'price'],
    });
  });
});
