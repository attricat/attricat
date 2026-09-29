import { AttributeValue } from './values/AttributeValue';
import type { ViewComponentDefinition } from './componentTypes';
import { VIEW_COMPONENT_IDS, VIEW_COMPONENT_VERSION } from '../constants';

export const fieldDisplayComponent = {
  id: VIEW_COMPONENT_IDS.fieldDisplay,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['display'],
  placements: ['field'],
  value_types: [
    'string',
    'number',
    'integer',
    'boolean',
    'date',
    'datetime',
    'time',
  ],
  allowed_props: [],
  valueRenderer: AttributeValue,
} satisfies ViewComponentDefinition;
