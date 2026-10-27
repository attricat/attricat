import type { DefinitionTemplate } from '../definition-editor/DefinitionTemplateDialog';
import choiceDefinition from './templates/choice.toml?raw';
import requiredDefinition from './templates/required.toml?raw';
import staleDefinition from './templates/stale.toml?raw';
import uniqueDefinition from './templates/unique.toml?raw';

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
];
