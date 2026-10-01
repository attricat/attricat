import i18n from '../../../i18n';
import { isEmailAddress } from '../email';
import { VIEW_COMPONENT_IDS, VIEW_COMPONENT_VERSION } from '../constants';
import type { ViewComponentDefinition } from './componentTypes';
import { EmailAttributeEditor } from './EmailInput';
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

export const emailEditComponent = {
  id: VIEW_COMPONENT_IDS.emailEdit,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['edit'],
  placements: ['field'],
  value_types: ['string'],
  allowed_props: [],
  valueEditor: EmailAttributeEditor,
  validateValue: (value: string) =>
    !value.trim() || isEmailAddress(value.trim())
      ? undefined
      : i18n.t('entities.invalidEmail'),
} satisfies ViewComponentDefinition;
