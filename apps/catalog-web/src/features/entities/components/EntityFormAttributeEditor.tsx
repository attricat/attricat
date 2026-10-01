import { useTranslation } from 'react-i18next';
import { savedStatusValue } from '../status';
import type { Attribute, ComponentReference, FormAttributeValue } from '../api';
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
  statusParentContextIds?: readonly string[];
  disabled?: boolean;
  defaultContextId: string | null;
  entityId?: string;
  existingValues: readonly FormAttributeValue[];
  statusSavedValues?: readonly FormAttributeValue[];
  fieldErrors: Record<string, string>;
  highlightedAttributes: readonly string[];
  migrationReviewMessages: Readonly<Record<string, string>>;
  resolvedValues: ResolvedFormValues;
};

type Props = EntityFormAttributeEditorContext & {
  attribute: Attribute;
  component?: ComponentReference | null;
  required?: boolean;
  onChange: (value: string) => void;
  value: string;
};

/** Renders one attribute editor with its context-aware helper text. */
export const EntityFormAttributeEditor = ({
  attribute,
  component,
  required,
  statusParentContextIds = [],
  disabled = false,
  contextId,
  defaultContextId,
  entityId,
  existingValues,
  statusSavedValues = existingValues,
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
      component={component}
      required={required}
      contextId={contextId}
      disabled={disabled || readonly || defaultOnly}
      statusBaseline={savedStatusValue(attribute, statusSavedValues, [
        contextId,
        ...(attribute.context_fallback === 'none'
          ? []
          : statusParentContextIds),
      ])}
      inheritedStatus={savedStatusValue(
        attribute,
        statusSavedValues,
        attribute.context_fallback === 'none' ? [] : statusParentContextIds,
      )}
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
