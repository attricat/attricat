// @vitest-environment jsdom
import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import '../../../i18n';
import type { Attribute } from '../api';
import { ENUM_LABELS_SCHEMA_KEY } from '../choices';
import { ConflictingChange } from './ConflictingChange';

const size: Attribute = {
  code: 'size',
  value_type: 'string',
  value_schema: {
    enum: ['s', 'l'],
    [ENUM_LABELS_SCHEMA_KEY]: { s: 'Small', l: 'Large' },
  },
};

afterEach(cleanup);

describe('ConflictingChange', () => {
  it('shows values as the record does', () => {
    const { container } = render(
      <ConflictingChange attribute={size} mine="s" theirs="l" />,
    );
    expect(container.textContent).toBe('size: yours “Small”, now “Large”');
  });

  it('says a value is empty without quoting it', () => {
    const inStock: Attribute = { code: 'in_stock', value_type: 'boolean' };
    const { container } = render(
      <ConflictingChange attribute={inStock} mine="true" theirs="" />,
    );
    expect(container.textContent).toBe('in stock: yours “Yes”, now empty');
  });
});
