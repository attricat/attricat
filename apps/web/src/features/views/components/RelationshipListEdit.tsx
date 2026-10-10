import type { ViewComponentDefinition } from './componentTypes';
import { VIEW_COMPONENT_IDS, VIEW_COMPONENT_VERSION } from '../constants';

// RecordForm provides the editor because it owns form state.
export const relationshipListEditComponent = {
  id: VIEW_COMPONENT_IDS.relationshipListEdit,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['edit'],
  placements: ['relationship_list'],
  value_types: ['relationship'],
  allowed_props: [],
} satisfies ViewComponentDefinition;
