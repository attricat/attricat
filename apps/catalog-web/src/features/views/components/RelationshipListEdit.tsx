import type { ViewComponentDefinition } from './component-types';

// EntityForm provides the editor because it owns form state.
export const relationshipListEditComponent = {
  id: 'catalog.relationship_list_edit',
  version: 1,
  capabilities: ['edit'],
  placements: ['relationship_list'],
  value_types: ['relationship'],
  allowed_props: [],
} satisfies ViewComponentDefinition;
