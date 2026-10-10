import { describe, expect, it } from 'vitest';
import { requiredRuleCodesFromInput } from './channelChecks';

describe('required rule codes', () => {
  it('trims, drops blanks and removes duplicates in entry order', () => {
    expect(
      requiredRuleCodesFromInput([' has-sku ', '', 'has-image', 'has-sku']),
    ).toEqual(['has-sku', 'has-image']);
  });
});
