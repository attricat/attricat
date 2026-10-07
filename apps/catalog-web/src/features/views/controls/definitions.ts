import type { ComponentType } from 'react';
import { VIEW_COMPONENT_IDS, VIEW_COMPONENT_VERSION } from '../constants';
import type {
  ValueEditorProps,
  ValueRenderer,
  ViewComponentDefinition,
} from '../components/componentTypes';
import { validateColor } from './color';
import {
  ColorEditor,
  EmailEditor,
  MarkdownFieldEditor,
  PhoneEditor,
  UrlEditor,
} from './editors';
import { validateEmail } from './email';
import { validateUrl } from './url';
import {
  ColorValue,
  EmailValue,
  MarkdownValue,
  PhoneValue,
  UrlValue,
} from './values';

/*
 * Opt-in controls for plain `string` attributes. They change presentation and
 * web-form validation only; persisted constraints belong in JSON Schema.
 */

const stringDisplay = (
  id: string,
  editComponentId: string,
  valueRenderer: ValueRenderer,
  placements: ViewComponentDefinition['placements'] = ['field', 'table'],
): ViewComponentDefinition => ({
  id,
  editComponentId,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['display'],
  placements,
  value_types: ['string'],
  allowed_props: [],
  valueRenderer,
});

const stringEdit = (
  id: string,
  valueEditor: ComponentType<ValueEditorProps>,
  behavior: Pick<
    ViewComponentDefinition,
    'validateValue' | 'preservesWhitespace'
  > = {},
): ViewComponentDefinition => ({
  id,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['edit'],
  placements: ['field'],
  value_types: ['string'],
  allowed_props: [],
  valueEditor,
  ...behavior,
});

export const colorDisplayComponent = stringDisplay(
  VIEW_COMPONENT_IDS.colorDisplay,
  VIEW_COMPONENT_IDS.colorEdit,
  ColorValue,
);
export const colorEditComponent = stringEdit(
  VIEW_COMPONENT_IDS.colorEdit,
  ColorEditor,
  { validateValue: validateColor },
);

export const emailDisplayComponent = stringDisplay(
  VIEW_COMPONENT_IDS.emailDisplay,
  VIEW_COMPONENT_IDS.emailEdit,
  EmailValue,
);
export const emailEditComponent = stringEdit(
  VIEW_COMPONENT_IDS.emailEdit,
  EmailEditor,
  { validateValue: validateEmail },
);

export const urlDisplayComponent = stringDisplay(
  VIEW_COMPONENT_IDS.urlDisplay,
  VIEW_COMPONENT_IDS.urlEdit,
  UrlValue,
);
export const urlEditComponent = stringEdit(
  VIEW_COMPONENT_IDS.urlEdit,
  UrlEditor,
  { validateValue: validateUrl },
);

export const phoneDisplayComponent = stringDisplay(
  VIEW_COMPONENT_IDS.phoneDisplay,
  VIEW_COMPONENT_IDS.phoneEdit,
  PhoneValue,
);
export const phoneEditComponent = stringEdit(
  VIEW_COMPONENT_IDS.phoneEdit,
  PhoneEditor,
);

export const markdownDisplayComponent = stringDisplay(
  VIEW_COMPONENT_IDS.markdownDisplay,
  VIEW_COMPONENT_IDS.markdownEdit,
  MarkdownValue,
  ['field'],
);
export const markdownEditComponent = stringEdit(
  VIEW_COMPONENT_IDS.markdownEdit,
  MarkdownFieldEditor,
  { preservesWhitespace: true },
);

/** Registered in this order; `contracts/view-components.json` must match. */
export const stringControlComponents = [
  colorDisplayComponent,
  colorEditComponent,
  emailDisplayComponent,
  emailEditComponent,
  urlDisplayComponent,
  urlEditComponent,
  phoneDisplayComponent,
  phoneEditComponent,
  markdownDisplayComponent,
  markdownEditComponent,
];
