import { AttributeValue } from './values/AttributeValue';
import type { ViewComponentDefinition } from './component-types';

export const fieldDisplayComponent = {
  id: 'catalog.field_display',
  version: 1,
  capabilities: ['display'],
  placements: ['field'],
  value_types: [
    'string',
    'number',
    'integer',
    'boolean',
    'date',
    'datetime',
    'time',
  ],
  allowed_props: [],
  valueRenderer: AttributeValue,
} satisfies ViewComponentDefinition;
