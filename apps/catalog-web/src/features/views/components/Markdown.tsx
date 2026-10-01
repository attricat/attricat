import { useTranslation } from 'react-i18next';
import { MarkdownContent } from '../../markdown/MarkdownContent';
import { MarkdownAttributeEditor } from '../../markdown/MarkdownEditor';
import { VIEW_COMPONENT_IDS, VIEW_COMPONENT_VERSION } from '../constants';
import type { ViewComponentDefinition, ValueRenderer } from './componentTypes';

const MarkdownValue: ValueRenderer = ({ value }) => {
  const { t } = useTranslation();
  return (
    <MarkdownContent
      value={typeof value === 'string' ? value : t('entities.notSet')}
    />
  );
};

export const markdownDisplayComponent = {
  id: VIEW_COMPONENT_IDS.markdownDisplay,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['display'],
  placements: ['field'],
  value_types: ['string'],
  allowed_props: [],
  valueRenderer: MarkdownValue,
} satisfies ViewComponentDefinition;

export const markdownEditComponent = {
  id: VIEW_COMPONENT_IDS.markdownEdit,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['edit'],
  placements: ['field'],
  value_types: ['string'],
  allowed_props: [],
  valueEditor: MarkdownAttributeEditor,
  preservesWhitespace: true,
} satisfies ViewComponentDefinition;
