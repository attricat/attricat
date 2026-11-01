// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import i18n from '../i18n';
import { notFoundError } from '../api/errors';
import { QueryErrorNotice } from './QueryErrorNotice';

describe('QueryErrorNotice', () => {
  beforeEach(() => i18n.changeLanguage('en'));

  it('offers a retry for a failure that may be a connection problem', () => {
    render(<QueryErrorNotice error={new Error('offline')} onRetry={vi.fn()} />);
    expect(
      screen.getByText('Check your connection and try again.'),
    ).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Try again' })).toBeTruthy();
  });

  it('says why a refused request failed without offering a retry', () => {
    render(<QueryErrorNotice error={notFoundError()} onRetry={vi.fn()} />);
    expect(
      screen.getByText("It doesn't exist, or you can't access it."),
    ).toBeTruthy();
    expect(screen.queryByRole('button')).toBeNull();
  });
});
