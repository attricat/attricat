import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { LoadMoreButton } from './LoadMoreButton';

describe('LoadMoreButton', () => {
  it('renders an enabled accessible default control', () => {
    const markup = renderToStaticMarkup(
      <LoadMoreButton onLoadMore={vi.fn()} />,
    );

    expect(markup).toContain('Load more');
    expect(markup).not.toMatch(/<button[^>]*\sdisabled(?:=|\s|>)/);
  });

  it('renders a disabled loading state', () => {
    const markup = renderToStaticMarkup(
      <LoadMoreButton isLoading onLoadMore={vi.fn()} />,
    );

    expect(markup).toContain('Loading...');
    expect(markup).toMatch(/<button[^>]*\sdisabled(?:=|\s|>)/);
    expect(markup).toContain('aria-busy="true"');
  });
});
