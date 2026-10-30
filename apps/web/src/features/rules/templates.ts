import type { DefinitionTemplate } from '../definition-editor/DefinitionTemplateDialog';
import choiceDefinition from './templates/choice.yaml?raw';
import expiryDefinition from './templates/expiry.yaml?raw';
import linkedDefinition from './templates/linked.yaml?raw';
import requiredDefinition from './templates/required.yaml?raw';
import staleDefinition from './templates/stale.yaml?raw';
import uniqueDefinition from './templates/unique.yaml?raw';

export const ruleTemplates: readonly DefinitionTemplate[] = [
  {
    definition: requiredDefinition,
    descriptionKey: 'rules.templates.requiredDescription',
    labelKey: 'rules.templates.required',
  },
  {
    definition: choiceDefinition,
    descriptionKey: 'rules.templates.choiceDescription',
    labelKey: 'rules.templates.choice',
  },
  {
    definition: staleDefinition,
    descriptionKey: 'rules.templates.staleDescription',
    labelKey: 'rules.templates.stale',
  },
  {
    definition: uniqueDefinition,
    descriptionKey: 'rules.templates.uniqueDescription',
    labelKey: 'rules.templates.unique',
  },
  {
    definition: expiryDefinition,
    descriptionKey: 'rules.templates.expiryDescription',
    labelKey: 'rules.templates.expiry',
  },
  {
    definition: linkedDefinition,
    descriptionKey: 'rules.templates.linkedDescription',
    labelKey: 'rules.templates.linked',
  },
];
