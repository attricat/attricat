import { useTranslation } from 'react-i18next';
import { savedStatusState } from '../status';
import type { Attribute, ComponentReference, FormAttributeValue } from '../api';
import type { StatusTransitionAccess } from '../recordControls';
import {
  filesForAttribute,
  formatResolvedValue,
  hasLocalFormValue,
  isDefaultContextOnly,
  type ResolvedFormValues,
} from '../entityFormAttributes';
import { attributeValueTypes } from '../valueTypes';
import { EntityAttributeEditor } from './EntityAttributeEditor';
import { AttributeValue } from '../../views/components/values/AttributeValue';

export type EntityFormAttributeEditorContext = {
  contextId: string | null;
  statusParentContextIds?: readonly string[];
  disabled?: boolean;
  defaultContextId: string | null;
  entityId?: string;
  existingValues: readonly FormAttributeValue[];
  statusSavedValues?: readonly FormAttributeValue[];
  /** Locked attribute codes mapped to the label of the status that locks them. */
  lockedAttributes?: Readonly<Record<string, string>>;
  statusTransitions?: readonly StatusTransitionAccess[];
  fieldErrors: Record<string, string>;
  highlightedAttributes: readonly string[];
  migrationReviewMessages: Readonly<Record<string, string>>;
  resolvedValues: ResolvedFormValues;
  onEntityUpdated?: (updatedAt: string) => void;
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
  lockedAttributes = {},
  statusTransitions = [],
  fieldErrors,
  highlightedAttributes,
  migrationReviewMessages,
  onChange,
  onEntityUpdated,
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
  const lockedBy = lockedAttributes[attribute.code];
  const savedStatus = savedStatusState(
    attribute,
    statusSavedValues,
    contextId,
    statusParentContextIds,
  );

  const inheritedHelperText = () => {
    if (!inherited) return undefined;
    const context = resolvedValue.source_context.code;
    return attribute.value_type === attributeValueTypes.relationship ||
      attribute.value_type === attributeValueTypes.file
      ? t('entities.inheritedFromContext', { context })
      : t('entities.inheritedValue', {
          context,
          value: formatResolvedValue(resolvedValue.value),
        });
  };
  const helperText = readonly
    ? t('entities.managedBySystem')
    : lockedBy !== undefined
      ? t('entities.lockedByStatus', { status: lockedBy })
      : defaultOnly
        ? t('entities.managedInDefault')
        : inheritedHelperText();

  return (
    <>
      {inherited && attribute.value_type === attributeValueTypes.file && (
        <AttributeValue attribute={attribute} value={resolvedValue.value} />
      )}
      <EntityAttributeEditor
        attribute={attribute}
        component={component}
        required={required}
        contextId={contextId}
        disabled={disabled || readonly || defaultOnly || lockedBy !== undefined}
        statusBaseline={savedStatus.current}
        inheritedStatus={savedStatus.inherited}
        statusTransitions={statusTransitions.filter(
          (edge) => edge.attribute_code === attribute.code,
        )}
        entityId={entityId}
        files={filesForAttribute(existingValues, attribute.code, contextId)}
        error={fieldErrors[attribute.code]}
        helperText={helperText}
        migrationReviewMessage={migrationReviewMessages[attribute.code]}
        onChange={onChange}
        onEntityUpdated={onEntityUpdated}
        showMigrationBadge={highlightedAttributes.includes(attribute.code)}
        value={value}
      />
    </>
  );
};
