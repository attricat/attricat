// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/react';
import type { MouseEventHandler, ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import type { EntityItem } from '../entities/api';
import { EntityDisplayCell } from './ExplorerTableCells';

vi.mock('@tanstack/react-router', () => ({
  Link: ({
    children,
    onClick,
  }: {
    children: ReactNode;
    onClick?: MouseEventHandler<HTMLAnchorElement>;
  }) => (
    <a href="#entity" onClick={onClick}>
      {children}
    </a>
  ),
}));

const entity = {
  display: { default: 'Sample product' },
  id: '123e4567-e89b-12d3-a456-426614174001',
  is_sample: false,
} as unknown as EntityItem;

describe('EntityDisplayCell', () => {
  it('opens the entity panel on a plain click instead of navigating', () => {
    const onOpenPanel = vi.fn();
    render(
      <EntityDisplayCell
        contextCodes={['default']}
        entity={entity}
        onOpenPanel={onOpenPanel}
      />,
    );
    const link = screen.getByRole('link', { name: 'Sample product' });

    const navigated = fireEvent.click(link);

    expect(navigated).toBe(false);
    expect(onOpenPanel).toHaveBeenCalledWith(entity.id, link);
  });

  it.each([{ ctrlKey: true }, { metaKey: true }, { shiftKey: true }])(
    'keeps the link default for a modified click (%o)',
    (modifier) => {
      const onOpenPanel = vi.fn();
      render(
        <EntityDisplayCell
          contextCodes={['default']}
          entity={entity}
          onOpenPanel={onOpenPanel}
        />,
      );

      const navigated = fireEvent.click(
        screen.getByRole('link', { name: 'Sample product' }),
        modifier,
      );

      expect(navigated).toBe(true);
      expect(onOpenPanel).not.toHaveBeenCalled();
    },
  );

  it('navigates to the entity page without a panel', () => {
    render(<EntityDisplayCell contextCodes={['default']} entity={entity} />);

    expect(
      fireEvent.click(screen.getByRole('link', { name: 'Sample product' })),
    ).toBe(true);
  });
});
