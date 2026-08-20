import { describe, expect, it } from 'vitest';
import type { Attribute, ViewDefinition } from '../../../entities/api';
import { EntityHeading } from './EntityHeading';

const attributes = [
  { code: 'title', value_type: 'string', context_fallback: 'default' },
] satisfies Attribute[];

const view = {
  type: 'stack',
  component: { id: 'catalog.entity_heading', version: 1, props: {} },
  children: [
    { type: 'field', field: 'title' },
    { type: 'text', text: 'Generated catalog product' },
  ],
} satisfies ViewDefinition;

describe('EntityHeading', () => {
  it('renders a configured field heading', () => {
    expect(() =>
      EntityHeading({
        attributes,
        entityId: 'entity-1',
        values: { title: { value: 'Product 001' } },
        view,
      }),
    ).not.toThrow();
  });
});
