import { describe, expect, it } from 'vitest';
import type { Attribute } from './api';
import {
  smartFillFormFields,
  unplacedRequiredAttributes,
} from './entityFormAttributes';

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

describe('unplacedRequiredAttributes', () => {
  const attributes = [
    { code: 'title', value_type: 'string' },
    { code: 'sku', value_type: 'string', tags: ['hidden:form'] },
    { code: 'internal', value_type: 'string', tags: ['hidden:detail'] },
    { code: 'secret', value_type: 'string', tags: ['hidden'] },
  ] satisfies Attribute[];
  const required = ['title', 'sku', 'internal', 'secret'];
  const codes = (view: Parameters<typeof unplacedRequiredAttributes>[1]) =>
    unplacedRequiredAttributes(attributes, view, required).map(
      (attribute) => attribute.code,
    );

  it('lists required fields a layout does not place', () => {
    expect(
      codes({ type: 'stack', children: [{ type: 'field', field: 'title' }] }),
    ).toEqual(['sku', 'internal', 'secret']);
  });

  it('lists required fields the default detail layout hides', () => {
    // The default layout already shows `sku`, which is only hidden from forms.
    expect(codes({ type: 'table', fields: [], columns: [] } as never)).toEqual([
      'internal',
      'secret',
    ]);
  });
});
