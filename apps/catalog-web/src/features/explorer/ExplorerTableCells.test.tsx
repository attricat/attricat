// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/react';
import type { MouseEventHandler, ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import type { RecordItem } from '../records/api';
import { recordPanelOpenerAttribute } from './constants';
import { RecordDisplayCell } from './ExplorerTableCells';

vi.mock('@tanstack/react-router', () => ({
  Link: ({
    children,
    onClick,
    ...props
  }: {
    children: ReactNode;
    onClick?: MouseEventHandler<HTMLAnchorElement>;
  }) => (
    <a
      // Keep only the data attributes; router props are not valid on <a>.
      {...Object.fromEntries(
        Object.entries(props).filter(([name]) => name.startsWith('data-')),
      )}
      href="#record"
      onClick={onClick}
    >
      {children}
    </a>
  ),
}));

const record = {
  display: { default: 'Sample product' },
  id: '123e4567-e89b-12d3-a456-426614174001',
  is_sample: false,
} as unknown as RecordItem;

describe('RecordDisplayCell', () => {
  it('opens the record panel on a plain click instead of navigating', () => {
    const onOpenPanel = vi.fn();
    render(
      <RecordDisplayCell
        contextCodes={['default']}
        record={record}
        onOpenPanel={onOpenPanel}
      />,
    );
    const link = screen.getByRole('link', { name: 'Sample product' });

    const navigated = fireEvent.click(link);

    expect(navigated).toBe(false);
    expect(onOpenPanel).toHaveBeenCalledWith(record.id, link);
    // Marks the link so clicking it switches the open panel instead of closing it.
    expect(link.hasAttribute(recordPanelOpenerAttribute)).toBe(true);
  });

  it.each([
    { ctrlKey: true },
    { metaKey: true },
    { shiftKey: true },
    { altKey: true },
    { button: 1 },
  ])('keeps the link default for a modified click (%o)', (modifier) => {
    const onOpenPanel = vi.fn();
    render(
      <RecordDisplayCell
        contextCodes={['default']}
        record={record}
        onOpenPanel={onOpenPanel}
      />,
    );

    const navigated = fireEvent.click(
      screen.getByRole('link', { name: 'Sample product' }),
      modifier,
    );

    expect(navigated).toBe(true);
    expect(onOpenPanel).not.toHaveBeenCalled();
  });

  it('navigates to the record page without a panel', () => {
    render(<RecordDisplayCell contextCodes={['default']} record={record} />);
    const link = screen.getByRole('link', { name: 'Sample product' });

    expect(fireEvent.click(link)).toBe(true);
    expect(link.hasAttribute(recordPanelOpenerAttribute)).toBe(false);
  });
});
