import { useTranslation } from 'react-i18next';
import { savedStatusState } from '../status';
import type {
  Attribute,
  ComponentReference,
  FormAttributeValue,
  StatusTransitionAccess,
} from '../api';
import {
  filesForAttribute,
  formatResolvedValue,
  hasLocalFormValue,
  isDefaultContextOnly,
  type ResolvedFormValues,
} from '../recordFormAttributes';
import { attributeValueTypes } from '../valueTypes';
import { RecordAttributeEditor } from './RecordAttributeEditor';
import { AttributeValue } from '../../views/components/values/AttributeValue';

export type RecordFormAttributeEditorContext = {
  contextId: string | null;
  statusParentContextIds?: readonly string[];
  disabled?: boolean;
  defaultContextId: string | null;
  recordId?: string;
  existingValues: readonly FormAttributeValue[];
  statusSavedValues?: readonly FormAttributeValue[];
  /** Locked attribute codes mapped to the label of the status that locks them. */
  lockedAttributes?: Readonly<Record<string, string>>;
  statusTransitions?: readonly StatusTransitionAccess[];
  fieldErrors: Record<string, string>;
  highlightedAttributes: readonly string[];
  migrationReviewMessages: Readonly<Record<string, string>>;
  resolvedValues: ResolvedFormValues;
  onRecordUpdated?: (updatedAt: string) => void;
};

type Props = RecordFormAttributeEditorContext & {
  attribute: Attribute;
  component?: ComponentReference | null;
  required?: boolean;
  onChange: (value: string) => void;
  value: string;
};

/** Renders one attribute editor with its context-aware helper text. */
export const RecordFormAttributeEditor = ({
  attribute,
  component,
  required,
  statusParentContextIds = [],
  disabled = false,
  contextId,
  defaultContextId,
  recordId,
  existingValues,
  statusSavedValues = existingValues,
  lockedAttributes = {},
  statusTransitions = [],
  fieldErrors,
  highlightedAttributes,
  migrationReviewMessages,
  onChange,
  onRecordUpdated,
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
      ? t('records.inheritedFromContext', { context })
      : t('records.inheritedValue', {
          context,
          value: formatResolvedValue(resolvedValue.value),
        });
  };
  const helperText = readonly
    ? t('records.managedBySystem')
    : lockedBy !== undefined
      ? t('records.lockedByStatus', { status: lockedBy })
      : defaultOnly
        ? t('records.managedInDefault')
        : inheritedHelperText();

  return (
    <>
      {inherited && attribute.value_type === attributeValueTypes.file && (
        <AttributeValue attribute={attribute} value={resolvedValue.value} />
      )}
      <RecordAttributeEditor
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
        recordId={recordId}
        files={filesForAttribute(existingValues, attribute.code, contextId)}
        error={fieldErrors[attribute.code]}
        helperText={helperText}
        migrationReviewMessage={migrationReviewMessages[attribute.code]}
        onChange={onChange}
        onRecordUpdated={onRecordUpdated}
        showMigrationBadge={highlightedAttributes.includes(attribute.code)}
        value={value}
      />
    </>
  );
};
