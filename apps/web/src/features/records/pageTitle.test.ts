import { describe, expect, it } from 'vitest';
import '../../i18n';
import type { Attribute } from './api';
import { pageTitleLabel } from './pageTitle';

describe('pageTitleLabel', () => {
  const attributes = [
    { code: 'name', value_type: 'string' },
    { code: 'sku', value_type: 'string' },
    { code: 'active', value_type: 'boolean' },
  ] as Attribute[];
  const views = {
    page_title: { type: 'page_title', fields: ['name', 'sku', 'active'] },
  };

  it('joins the set page_title fields as text', () => {
    expect(
      pageTitleLabel(views, attributes, {
        name: { value: 'Blue lamp' },
        sku: { value: null },
        active: { value: true },
      }),
    ).toBe('Blue lamp · Yes');
  });

  it('is null without a page_title view or any set field', () => {
    expect(
      pageTitleLabel({}, attributes, { name: { value: 'Blue lamp' } }),
    ).toBeNull();
    expect(pageTitleLabel(views, attributes, {})).toBeNull();
  });

  it('leaves out empty text fields', () => {
    expect(
      pageTitleLabel(views, attributes, {
        name: { value: '' },
        sku: { value: 'LAMP-1' },
      }),
    ).toBe('LAMP-1');
    expect(
      pageTitleLabel(views, attributes, {
        name: { value: ' ' },
        sku: { value: '' },
      }),
    ).toBeNull();
  });
});
