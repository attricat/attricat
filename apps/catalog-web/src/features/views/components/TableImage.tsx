import type { ViewComponentDefinition } from './componentTypes';
import { VIEW_COMPONENT_IDS, VIEW_COMPONENT_VERSION } from '../constants';
import { TableImageValue } from './values/TableImageValue';

export const tableImageComponent = {
  id: VIEW_COMPONENT_IDS.tableImage,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['display'],
  placements: ['table'],
  value_types: ['file'],
  allowed_props: [],
  valueRenderer: TableImageValue,
} satisfies ViewComponentDefinition;
