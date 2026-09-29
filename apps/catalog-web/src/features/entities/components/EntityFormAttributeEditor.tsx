import { useTranslation } from 'react-i18next';
import type { Attribute, FormAttributeValue } from '../api';
import {
  filesForAttribute,
  formatResolvedValue,
  hasLocalFormValue,
  isDefaultContextOnly,
  type ResolvedFormValues,
} from '../entityFormAttributes';
import { attributeValueTypes } from '../valueTypes';
import { EntityAttributeEditor } from './EntityAttributeEditor';

export type EntityFormAttributeEditorContext = {
  contextId: string | null;
  defaultContextId: string | null;
  entityId?: string;
  existingValues: readonly FormAttributeValue[];
  fieldErrors: Record<string, string>;
  highlightedAttributes: readonly string[];
  migrationReviewMessages: Readonly<Record<string, string>>;
  resolvedValues: ResolvedFormValues;
};

type Props = EntityFormAttributeEditorContext & {
  attribute: Attribute;
  onChange: (value: string) => void;
  value: string;
};

/** Renders one attribute editor with its context-aware helper text. */
export const EntityFormAttributeEditor = ({
  attribute,
  contextId,
  defaultContextId,
  entityId,
  existingValues,
  fieldErrors,
  highlightedAttributes,
  migrationReviewMessages,
  onChange,
  resolvedValues,
  value,
}: Props) => {
  const { t } = useTranslation();
  const resolvedValue = resolvedValues[attribute.code];
  const inherited =
    !hasLocalFormValue(existingValues, attribute.code, contextId) &&
    resolvedValue !== undefined &&
    resolvedValue.source_context.id !== contextId;
  const defaultOnly = isDefaultContextOnly(
    attribute,
    contextId,
    defaultContextId,
  );
  const readonly = attribute.readonly === true;

  const inheritedHelperText = () => {
    if (!inherited) return undefined;
    const context = resolvedValue.source_context.code;
    return attribute.value_type === attributeValueTypes.relationship
      ? t('entities.inheritedFromContext', { context })
      : t('entities.inheritedValue', {
          context,
          value: formatResolvedValue(resolvedValue.value),
        });
  };
  const helperText = readonly
    ? t('entities.managedBySystem')
    : defaultOnly
      ? t('entities.managedInDefault')
      : inheritedHelperText();

  return (
    <EntityAttributeEditor
      attribute={attribute}
      contextId={contextId}
      disabled={readonly || defaultOnly}
      entityId={entityId}
      files={filesForAttribute(existingValues, attribute.code, contextId)}
      error={fieldErrors[attribute.code]}
      helperText={helperText}
      migrationReviewMessage={migrationReviewMessages[attribute.code]}
      onChange={onChange}
      showMigrationBadge={highlightedAttributes.includes(attribute.code)}
      value={value}
    />
  );
};
