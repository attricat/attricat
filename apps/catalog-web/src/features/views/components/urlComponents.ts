import { VIEW_COMPONENT_IDS, VIEW_COMPONENT_VERSION } from '../constants';
import type { ViewComponentDefinition } from './componentTypes';
import { UrlDisplay } from './UrlDisplay';

export const urlDisplayComponent = {
  id: VIEW_COMPONENT_IDS.urlDisplay,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['display'],
  placements: ['field', 'table'],
  value_types: ['string'],
  allowed_props: [],
  valueRenderer: UrlDisplay,
} satisfies ViewComponentDefinition;

// EntityForm owns editor state; the selected reference is passed to its editor.
export const urlEditComponent = {
  id: VIEW_COMPONENT_IDS.urlEdit,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['edit'],
  placements: ['field'],
  value_types: ['string'],
  allowed_props: [],
} satisfies ViewComponentDefinition;
