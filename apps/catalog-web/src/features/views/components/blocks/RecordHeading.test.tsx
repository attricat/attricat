import { describe, expect, it } from 'vitest';
import type { Attribute, ViewDefinition } from '../../../records/api';
import { RecordHeading } from './RecordHeading';

const attributes = [
  { code: 'title', value_type: 'string', context_fallback: 'default' },
] satisfies Attribute[];

const view = {
  type: 'stack',
  component: { id: 'catalog.record_heading', version: 1, props: {} },
  children: [
    { type: 'field', field: 'title' },
    { type: 'text', text: 'Generated catalog product' },
  ],
} satisfies ViewDefinition;

describe('RecordHeading', () => {
  it('renders a configured field heading', () => {
    expect(() =>
      RecordHeading({
        attributes,
        recordId: 'record-1',
        values: { title: { value: 'Product 001' } },
        view,
      }),
    ).not.toThrow();
  });
});
