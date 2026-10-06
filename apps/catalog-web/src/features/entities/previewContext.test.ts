import { describe, expect, it } from 'vitest';
import type { Attribute } from './api';
import { contextAncestorCodes, resolvePreviewContext } from './previewContext';

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
        [
          {
            id: '00000000-0000-0000-0000-000000000001',
            code: 'default',
            data: {},
            parent_id: null,
          },
          {
            id: '00000000-0000-0000-0000-000000000002',
            code: 'en_GB',
            data: {},
            parent_id: '00000000-0000-0000-0000-000000000001',
          },
        ],
      ),
    ).toEqual({ title: 'Blue shirt (UK)' });
  });

  it('uses default-enabled fields when the selected context is absent', () => {
    expect(
      resolvePreviewContext(
        { default: { title: 'Blue shirt', regional_notice: 'UK terms' } },
        'en_GB',
        attributes,
        [
          {
            id: '00000000-0000-0000-0000-000000000001',
            code: 'default',
            data: {},
            parent_id: null,
          },
        ],
      ),
    ).toEqual({ title: 'Blue shirt' });
  });
});

describe('contextAncestorCodes', () => {
  it('lists a context and its ancestors, nearest first', () => {
    const contexts = [
      { id: 'a', code: 'default', data: {}, parent_id: null },
      { id: 'b', code: 'pl', data: {}, parent_id: 'a' },
      { id: 'c', code: 'pl-web', data: {}, parent_id: 'b' },
    ];
    expect(contextAncestorCodes(contexts, 'pl-web')).toEqual([
      'pl-web',
      'pl',
      'default',
    ]);
    expect(contextAncestorCodes(contexts, 'missing')).toEqual([]);
  });
});
