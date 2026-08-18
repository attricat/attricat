import type { ViewComponentDefinition } from './component-types';

// EntityForm provides the editor because it owns form state.
export const fieldEditComponent = {
  id: 'catalog.field_edit',
  version: 1,
  capabilities: ['edit'],
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
} satisfies ViewComponentDefinition;
