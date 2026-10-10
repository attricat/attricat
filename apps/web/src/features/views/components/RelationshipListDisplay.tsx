import { AttributeValue } from './values/AttributeValue';
import type { ViewComponentDefinition } from './componentTypes';
import { VIEW_COMPONENT_IDS, VIEW_COMPONENT_VERSION } from '../constants';

export const relationshipListDisplayComponent = {
  id: VIEW_COMPONENT_IDS.relationshipListDisplay,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['display'],
  placements: ['relationship_list'],
  value_types: ['relationship'],
  allowed_props: [],
  valueRenderer: AttributeValue,
} satisfies ViewComponentDefinition;
