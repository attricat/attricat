import type { DefinitionTemplate } from '../definition-editor/DefinitionTemplateDialog';
import assigneeDefinition from './templates/assignee.yaml?raw';
import choiceDefinition from './templates/choice.yaml?raw';
import imageDefinition from './templates/image.yaml?raw';
import numberDefinition from './templates/number.yaml?raw';
import statusDefinition from './templates/status.yaml?raw';
import textDefinition from './templates/text.yaml?raw';

export const reusableAttributeTemplates: readonly DefinitionTemplate[] = [
  {
    definition: textDefinition,
    descriptionKey: 'reusableAttributes.templates.textDescription',
    labelKey: 'reusableAttributes.templates.text',
  },
  {
    definition: numberDefinition,
    descriptionKey: 'reusableAttributes.templates.numberDescription',
    labelKey: 'reusableAttributes.templates.number',
  },
  {
    definition: choiceDefinition,
    descriptionKey: 'reusableAttributes.templates.choiceDescription',
    labelKey: 'reusableAttributes.templates.choice',
  },
  {
    definition: statusDefinition,
    descriptionKey: 'reusableAttributes.templates.statusDescription',
    labelKey: 'reusableAttributes.templates.status',
  },
  {
    definition: assigneeDefinition,
    descriptionKey: 'reusableAttributes.templates.assigneeDescription',
    labelKey: 'reusableAttributes.templates.assignee',
  },
  {
    definition: imageDefinition,
    descriptionKey: 'reusableAttributes.templates.imageDescription',
    labelKey: 'reusableAttributes.templates.image',
  },
];
