import {
  scalarValueTypes,
  type ViewComponentDefinition,
} from './componentTypes';
import { VIEW_COMPONENT_IDS, VIEW_COMPONENT_VERSION } from '../constants';

export const tableDisplayComponent = {
  id: VIEW_COMPONENT_IDS.tableDisplay,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['display'],
  placements: ['table'],
  value_types: scalarValueTypes,
  allowed_props: [],
} satisfies ViewComponentDefinition;
