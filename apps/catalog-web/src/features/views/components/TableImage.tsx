import type { ViewComponentDefinition } from './componentTypes';

export const tableImageComponent = {
  id: 'catalog.table_image',
  version: 1,
  capabilities: ['display'],
  placements: ['table'],
  value_types: ['file'],
  allowed_props: [],
} satisfies ViewComponentDefinition;
