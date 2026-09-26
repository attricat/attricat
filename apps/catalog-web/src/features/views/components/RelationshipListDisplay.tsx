import { AttributeValue } from './values/AttributeValue';
import type { ViewComponentDefinition } from './componentTypes';

export const relationshipListDisplayComponent = {
  id: 'catalog.relationship_list_display',
  version: 1,
  capabilities: ['display'],
  placements: ['relationship_list'],
  value_types: ['relationship'],
  allowed_props: [],
  valueRenderer: AttributeValue,
} satisfies ViewComponentDefinition;
