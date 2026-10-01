// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { CommentMarkdown } from './CommentMarkdown';
import { safeCommentUrl } from './markdown';
import { commentInputSchema } from './schemas';

describe('comment Markdown', () => {
  it('renders formatting without HTML or remote images', () => {
    const { container } = render(
      <CommentMarkdown
        body={
          '**Bold**\n\n<script>alert(1)</script>\n\n![Description](https://tracker.test/pixel)\n\n[Safe](https://example.test)\n\n[Unsafe](javascript:alert)'
        }
      />,
    );
    expect(screen.getByText('Bold').tagName).toBe('STRONG');
    expect(container.querySelector('script')).toBeNull();
    expect(container.querySelector('img')).toBeNull();
    expect(screen.getByText('Safe').getAttribute('href')).toBe(
      'https://example.test',
    );
    expect(screen.getByText('Unsafe').closest('a')).toBeNull();
  });

  it('rejects executable and protocol-relative links', () => {
    for (const value of [
      'javascript:alert(1)',
      'data:text/html,x',
      '//tracker.test',
      '/\\tracker.test',
    ]) {
      expect(safeCommentUrl(value)).toBe('');
    }
    expect(safeCommentUrl('/entities/123')).toBe('/entities/123');
  });

  it('validates code points, blanks and nulls', () => {
    expect(
      commentInputSchema.safeParse({ body: '🦀'.repeat(10000) }).success,
    ).toBe(true);
    for (const body of ['', ' \n', '\0', '🦀'.repeat(10001)]) {
      expect(commentInputSchema.safeParse({ body }).success).toBe(false);
    }
  });
});
