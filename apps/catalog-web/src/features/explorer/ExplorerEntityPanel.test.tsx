// @vitest-environment jsdom
import { act, fireEvent, render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import type { EntityPreviewFrame } from '../entities/components/EntityPreview';
import { entityPanelOpenerAttribute } from './constants';
import { ExplorerEntityPanel } from './ExplorerEntityPanel';

const entityId = '123e4567-e89b-12d3-a456-426614174001';
const copyId = '123e4567-e89b-12d3-a456-426614174002';

const preview = vi.hoisted(() => ({
  blueprint: undefined as EntityPreviewFrame['blueprint'],
}));

vi.mock('../entities/components/EntityPreview', () => ({
  EntityPreview: ({
    onDeleted,
    onDuplicated,
    renderFrame,
  }: {
    onDeleted: () => void;
    onDuplicated: (copy: { id: string }) => void;
    renderFrame: (frame: EntityPreviewFrame) => ReactNode;
  }) =>
    renderFrame({
      blueprint: preview.blueprint,
      headerActions: null,
      children: (
        <>
          <input aria-label="Title" />
          <button onClick={onDeleted} type="button">
            Delete
          </button>
          <button onClick={() => onDuplicated({ id: copyId })} type="button">
            Duplicate
          </button>
          {/* Stands in for a portalled menu or select backdrop. */}
          {createPortal(
            <button type="button">Portalled option</button>,
            document.body,
          )}
        </>
      ),
    }),
}));

vi.mock('../entity-comments/EntityCommentsLink', () => ({
  EntityCommentsLink: () => null,
}));

vi.mock('../../components/RouterLink', () => ({
  RouterButton: ({ children }: { children: ReactNode }) => (
    <a href="#entity">{children}</a>
  ),
}));

/** ClickAwayListener starts listening one tick after it mounts. */
const activateClickAway = () =>
  act(() => new Promise((resolve) => setTimeout(resolve, 0)));

const renderPanel = () => {
  const onClose = vi.fn();
  const onOpenEntity = vi.fn();
  render(
    <>
      <button type="button">Outside</button>
      <a href="#other" {...{ [entityPanelOpenerAttribute]: true }}>
        Other result
      </a>
      <ExplorerEntityPanel
        entityId={entityId}
        onClose={onClose}
        onOpenEntity={onOpenEntity}
      />
    </>,
  );
  return { onClose, onOpenEntity };
};

describe('ExplorerEntityPanel', () => {
  it('names the panel after the blueprint and moves focus into it', () => {
    preview.blueprint = {
      code: 'product',
      id: '123e4567-e89b-12d3-a456-426614174000',
      name: 'Product',
      version: 1,
    };
    renderPanel();
    preview.blueprint = undefined;

    const panel = screen.getByRole('complementary', { name: 'Product record' });
    expect(document.activeElement).toBe(panel);
  });

  it('falls back to a generic name before the blueprint loads', () => {
    renderPanel();

    expect(screen.getByRole('complementary', { name: 'Record' })).toBeTruthy();
  });

  it('closes from its close button and on Escape outside fields', () => {
    const { onClose } = renderPanel();

    fireEvent.click(screen.getByRole('button', { name: 'Close record panel' }));
    fireEvent.keyDown(screen.getByRole('complementary'), { key: 'Escape' });

    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it('leaves Escape to a field being edited', () => {
    const { onClose } = renderPanel();

    fireEvent.keyDown(screen.getByRole('textbox', { name: 'Title' }), {
      key: 'Escape',
    });

    expect(onClose).not.toHaveBeenCalled();
  });

  it('ignores Escape from portalled content opened in the panel', () => {
    const { onClose } = renderPanel();

    fireEvent.keyDown(
      screen.getByRole('button', { name: 'Portalled option' }),
      { key: 'Escape' },
    );

    expect(onClose).not.toHaveBeenCalled();
  });

  it('closes on a click outside but not on another result', async () => {
    const { onClose } = renderPanel();
    await activateClickAway();

    fireEvent.click(screen.getByRole('link', { name: 'Other result' }));
    expect(onClose).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: 'Outside' }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('stays open when a press inside ends outside, but only for that click', async () => {
    const { onClose } = renderPanel();
    await activateClickAway();
    const outside = screen.getByRole('button', { name: 'Outside' });

    fireEvent.pointerDown(screen.getByRole('textbox', { name: 'Title' }));
    fireEvent.click(outside);
    expect(onClose).not.toHaveBeenCalled();

    // A keyboard click outside has no press of its own.
    fireEvent.click(outside);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('keeps portalled content opened from the panel inside it', async () => {
    const { onClose } = renderPanel();
    await activateClickAway();

    fireEvent.click(screen.getByRole('button', { name: 'Portalled option' }));

    expect(onClose).not.toHaveBeenCalled();
  });

  it('shows a duplicate in the panel and closes once the record is deleted', () => {
    const { onClose, onOpenEntity } = renderPanel();

    fireEvent.click(screen.getByRole('button', { name: 'Duplicate' }));
    expect(onOpenEntity).toHaveBeenCalledWith(copyId);

    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('starts each record with fresh state', () => {
    const panel = (id: string) => (
      <ExplorerEntityPanel
        entityId={id}
        onClose={vi.fn()}
        onOpenEntity={vi.fn()}
      />
    );
    const { rerender } = render(panel(entityId));
    fireEvent.change(screen.getByRole('textbox', { name: 'Title' }), {
      target: { value: 'Unsaved' },
    });

    rerender(panel(copyId));

    expect(
      screen.getByRole<HTMLInputElement>('textbox', { name: 'Title' }).value,
    ).toBe('');
  });
});
