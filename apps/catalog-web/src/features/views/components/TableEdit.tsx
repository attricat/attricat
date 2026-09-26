import type { ViewComponentDefinition } from './componentTypes';

export const tableEditComponent = {
  id: 'catalog.table_edit',
  version: 1,
  capabilities: ['edit'],
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
