import {
  scalarValueTypes,
  type ViewComponentDefinition,
} from './componentTypes';
import { VIEW_COMPONENT_IDS, VIEW_COMPONENT_VERSION } from '../constants';

export const tableEditComponent = {
  id: VIEW_COMPONENT_IDS.tableEdit,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['edit'],
  placements: ['table'],
  value_types: scalarValueTypes,
  allowed_props: [],
} satisfies ViewComponentDefinition;
