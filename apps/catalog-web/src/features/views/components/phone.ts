import { VIEW_COMPONENT_IDS, VIEW_COMPONENT_VERSION } from '../constants';
import type { ViewComponentDefinition } from './componentTypes';
import { PhoneAttributeEditor } from './PhoneInput';
import { PhoneValue } from './values/PhoneValue';

export const phoneDisplayComponent = {
  id: VIEW_COMPONENT_IDS.phoneDisplay,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['display'],
  placements: ['field', 'table'],
  value_types: ['string'],
  allowed_props: [],
  valueRenderer: PhoneValue,
} satisfies ViewComponentDefinition;

export const phoneEditComponent = {
  id: VIEW_COMPONENT_IDS.phoneEdit,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['edit'],
  placements: ['field'],
  value_types: ['string'],
  allowed_props: [],
  valueEditor: PhoneAttributeEditor,
} satisfies ViewComponentDefinition;
