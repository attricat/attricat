import { describe, expect, it } from 'vitest';
import { withUnusedBlueprintCode } from './blueprintEditorUtils';

describe('withUnusedBlueprintCode', () => {
  const template = 'format_version: 1\ncode: product\nname: Product\n';

  it('keeps a free code', () => {
    expect(withUnusedBlueprintCode(template, new Set(['other']))).toBe(
      template,
    );
  });

  it('numbers a code that blueprints already use', () => {
    expect(
      withUnusedBlueprintCode(template, new Set(['product', 'product_2'])),
    ).toBe('format_version: 1\ncode: product_3\nname: Product\n');
  });
});
