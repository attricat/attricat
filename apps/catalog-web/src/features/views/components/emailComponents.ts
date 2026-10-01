import { VIEW_COMPONENT_IDS, VIEW_COMPONENT_VERSION } from '../constants';
import type { ViewComponentDefinition } from './componentTypes';
import { EmailValue } from './values/EmailValue';

export const emailDisplayComponent = {
  id: VIEW_COMPONENT_IDS.emailDisplay,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['display'],
  placements: ['field', 'table'],
  value_types: ['string'],
  allowed_props: [],
  valueRenderer: EmailValue,
} satisfies ViewComponentDefinition;

// EntityForm owns submitted values, validation and draft restoration.
export const emailEditComponent = {
  id: VIEW_COMPONENT_IDS.emailEdit,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['edit'],
  placements: ['field'],
  value_types: ['string'],
  allowed_props: [],
} satisfies ViewComponentDefinition;
