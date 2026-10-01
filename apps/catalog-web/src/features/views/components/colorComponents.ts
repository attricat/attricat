import i18n from '../../../i18n';
import { ColorAttributeEditor } from '../../entities/components/ColorAttributeEditor';
import { parseColor } from '../colorValue';
import { VIEW_COMPONENT_IDS, VIEW_COMPONENT_VERSION } from '../constants';
import type { ViewComponentDefinition } from './componentTypes';
import { ColorValue } from './values/ColorValue';

export const colorDisplayComponent = {
  id: VIEW_COMPONENT_IDS.colorDisplay,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['display'],
  placements: ['field', 'table'],
  value_types: ['string'],
  allowed_props: [],
  valueRenderer: ColorValue,
} satisfies ViewComponentDefinition;

export const colorEditComponent = {
  id: VIEW_COMPONENT_IDS.colorEdit,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['edit'],
  placements: ['field'],
  value_types: ['string'],
  allowed_props: [],
  valueEditor: ColorAttributeEditor,
  validateValue: (value: string) =>
    value === '' || parseColor(value)
      ? undefined
      : i18n.t('views.colorInvalid'),
} satisfies ViewComponentDefinition;
