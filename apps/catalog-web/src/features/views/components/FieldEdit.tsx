import {
  scalarValueTypes,
  type ViewComponentDefinition,
} from './componentTypes';
import { VIEW_COMPONENT_IDS, VIEW_COMPONENT_VERSION } from '../constants';

// EntityForm provides the editor because it owns form state.
export const fieldEditComponent = {
  id: VIEW_COMPONENT_IDS.fieldEdit,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['edit'],
  placements: ['field'],
  value_types: scalarValueTypes,
  allowed_props: [],
} satisfies ViewComponentDefinition;
