import { describe, expect, it } from 'vitest';
import type { Attribute } from './api';
import { resolvePreviewContext } from './preview-context';

const attributes = [
  { code: 'title', value_type: 'string', context_fallback: 'default' },
  { code: 'regional_notice', value_type: 'string', context_fallback: 'none' },
] satisfies Attribute[];

describe('preview context resolution', () => {
  it('resolves missing fields individually using their fallback policy', () => {
    expect(
      resolvePreviewContext(
        {
          default: { title: 'Blue shirt', regional_notice: 'UK terms' },
          en_GB: { title: 'Blue shirt (UK)' },
        },
        'en_GB',
        attributes,
      ),
    ).toEqual({ title: 'Blue shirt (UK)' });
  });

  it('uses default-enabled fields when the selected context is absent', () => {
    expect(
      resolvePreviewContext(
        { default: { title: 'Blue shirt', regional_notice: 'UK terms' } },
        'en_GB',
        attributes,
      ),
    ).toEqual({ title: 'Blue shirt' });
  });
});
