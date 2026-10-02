import { describe, expect, it } from 'vitest';
import { commentInputSchema } from './schemas';

describe('comment input', () => {
  it('validates code points, blanks and nulls', () => {
    expect(
      commentInputSchema.safeParse({ body: '🦀'.repeat(10000) }).success,
    ).toBe(true);
    for (const body of ['', ' \n', '\0', '🦀'.repeat(10001)]) {
      expect(commentInputSchema.safeParse({ body }).success).toBe(false);
    }
  });
});
