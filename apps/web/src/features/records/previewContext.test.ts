import { describe, expect, it } from 'vitest';
import { contextAncestorCodes } from './previewContext';

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
