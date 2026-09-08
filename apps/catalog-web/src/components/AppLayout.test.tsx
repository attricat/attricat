import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import '../i18n';
import { SessionErrorState } from './AppLayout';

describe('SessionErrorState', () => {
  it('renders an accessible session error with a retry control', () => {
    const markup = renderToStaticMarkup(
      <SessionErrorState onRetry={vi.fn()} />,
    );

    expect(markup).toContain('role="alert"');
    expect(markup).toContain('We couldn&#x27;t restore your session');
    expect(markup).toContain('Retry');
    expect(markup).toMatch(/<button[^>]*>Retry<\/button>/);
  });
});
