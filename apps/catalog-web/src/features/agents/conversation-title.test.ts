import { describe, expect, it } from 'vitest';
import { conversationTitleFromFirstMessage } from './conversation-title';

describe('conversationTitleFromFirstMessage', () => {
  it('normalizes the first message into a readable title', () => {
    expect(
      conversationTitleFromFirstMessage('  Review\n  product data  '),
    ).toBe('Review product data');
  });

  it('keeps generated titles short', () => {
    const title = conversationTitleFromFirstMessage('a'.repeat(100));
    expect(title).toHaveLength(72);
    expect(title).toMatch(/…$/);
  });
});
