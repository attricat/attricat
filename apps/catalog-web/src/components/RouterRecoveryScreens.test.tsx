import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import '../i18n';
import { QueryErrorNotice } from './QueryErrorNotice';
import { NotFoundScreen, RouterErrorScreen } from './RouterRecoveryScreens';

describe('router recovery screens', () => {
  it('offers retry and home actions for route failures', () => {
    const markup = renderToStaticMarkup(
      <RouterErrorScreen onRetry={vi.fn()} />,
    );

    expect(markup).toContain('Something went wrong');
    expect(markup).toContain('Try again');
    expect(markup).toContain('href="/"');
  });

  it('explains an unmatched route and provides a way home', () => {
    const markup = renderToStaticMarkup(<NotFoundScreen />);

    expect(markup).toContain('Page not found');
    expect(markup).toContain('Go to catalog home');
    expect(markup).toContain('href="/"');
  });

  it('renders a retryable, non-technical query error', () => {
    const markup = renderToStaticMarkup(
      <QueryErrorNotice
        error={new Error('secret server detail')}
        onRetry={vi.fn()}
      />,
    );

    expect(markup).toContain('Unable to load this information');
    expect(markup).toContain('Try again');
    expect(markup).not.toContain('secret server detail');
  });
});
