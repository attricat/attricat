import { AttributeValue } from './values/AttributeValue';
import {
  scalarValueTypes,
  type ViewComponentDefinition,
} from './componentTypes';
import { VIEW_COMPONENT_IDS, VIEW_COMPONENT_VERSION } from '../constants';

export const fieldDisplayComponent = {
  id: VIEW_COMPONENT_IDS.fieldDisplay,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['display'],
  placements: ['field'],
  value_types: scalarValueTypes,
  allowed_props: [],
  valueRenderer: AttributeValue,
} satisfies ViewComponentDefinition;
