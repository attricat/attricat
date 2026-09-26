import type { ViewComponentDefinition } from './componentTypes';

export const tableDisplayComponent = {
  id: 'catalog.table_display',
  version: 1,
  capabilities: ['display'],
  placements: ['table'],
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
} satisfies ViewComponentDefinition;
