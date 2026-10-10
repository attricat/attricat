// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../i18n';
import { LoadMoreButton } from './LoadMoreButton';

describe('LoadMoreButton', () => {
  it('is an accessible button that invokes its action', async () => {
    const onLoadMore = vi.fn();
    const user = userEvent.setup();

    render(<LoadMoreButton onLoadMore={onLoadMore} />);

    await user.click(screen.getByRole('button', { name: 'Load more' }));

    expect(onLoadMore).toHaveBeenCalledOnce();
  });

  it('exposes loading state and prevents duplicate actions', () => {
    const onLoadMore = vi.fn();

    render(<LoadMoreButton isLoading onLoadMore={onLoadMore} />);

    const button = screen.getByRole('button', { name: 'Loading...' });
    expect(button.getAttribute('aria-busy')).toBe('true');
    expect(button).toHaveProperty('disabled', true);

    expect(onLoadMore).not.toHaveBeenCalled();
  });
});
