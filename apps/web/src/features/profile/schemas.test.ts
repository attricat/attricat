import { describe, expect, it } from 'vitest';
import '../../i18n';
import { tokenInputSchema } from './schemas';

describe('tokenInputSchema', () => {
  it('explains a token without permissions in words', () => {
    const result = tokenInputSchema.safeParse({
      label: 'Nightly import',
      permissions: [],
    });
    expect(result.error?.issues[0]?.message).toBe(
      'Choose at least one permission.',
    );
  });
});
