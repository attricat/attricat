import i18n from '../../../i18n';
import { safeUrl } from '../urlPolicy';
import { VIEW_COMPONENT_IDS, VIEW_COMPONENT_VERSION } from '../constants';
import type { ViewComponentDefinition } from './componentTypes';
import { UrlDisplay } from './UrlDisplay';
import { UrlAttributeEditor } from './UrlEditor';

export const urlDisplayComponent = {
  id: VIEW_COMPONENT_IDS.urlDisplay,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['display'],
  placements: ['field', 'table'],
  value_types: ['string'],
  allowed_props: [],
  valueRenderer: UrlDisplay,
} satisfies ViewComponentDefinition;

export const urlEditComponent = {
  id: VIEW_COMPONENT_IDS.urlEdit,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['edit'],
  placements: ['field'],
  value_types: ['string'],
  allowed_props: [],
  valueEditor: UrlAttributeEditor,
  validateValue: (value: string) =>
    !value || safeUrl(value) ? undefined : i18n.t('views.invalidUrl'),
} satisfies ViewComponentDefinition;
