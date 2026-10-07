import { describe, expect, it } from 'vitest';
import type { Attribute } from './api';
import { smartFillFormFields } from './entityFormAttributes';

describe('smartFillFormFields', () => {
  it('keeps suggestions for editable scalar fields only', () => {
    const editable = [
      { code: 'title', value_type: 'string' },
      { code: 'related', value_type: 'relationship' },
      { code: 'manual', value_type: 'file' },
    ] satisfies Attribute[];

    expect(
      smartFillFormFields(editable, {
        title: 'New title',
        related: 'target',
        manual: 'file',
        secret: 'Not editable',
      }),
    ).toEqual({ title: 'New title' });
  });
});
