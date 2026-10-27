import { Box, Button, type SxProps, type Theme } from '@mui/material';
import { Undo2Icon } from 'lucide-react';
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
  hasLocalFormValue,
  isDefaultContextOnly,
} from '../recordFormAttributes';
import { defaultContextCode } from '../../contexts/constants';
import type { ResolvedValue } from '../../views/components/ValueField';
import { attributeValueTypes } from '../valueTypes';
import { attributeContextFallbacks } from '../constants';
import { RecordAttributeEditor } from './RecordAttributeEditor';
import { AttributeValue } from '../../views/components/values/AttributeValue';
import type { MigrationFieldReview } from '../migrationReview';
import { attributeHint } from '../attributeHints';
import { formatAttributeValue } from '../../views/components/values/formatAttributeValue';
import { useInstantFormat } from '../../../time/useInstantFormat';

/**
 * A value that cannot be changed here is still content: show it in secondary
 * text rather than the faded disabled colour, and leave the reason in the
 * helper text.
 */
const protectedValueSx: SxProps<Theme> = {
  display: 'contents',
  '& .MuiInputBase-root.Mui-disabled, & .MuiFormLabel-root.Mui-disabled, & .MuiFormControlLabel-label.Mui-disabled, & .MuiToggleButton-root.Mui-disabled.Mui-selected':
    { color: 'text.secondary' },
  '& .MuiInputBase-input.Mui-disabled, & .MuiSelect-select.Mui-disabled': {
    WebkitTextFillColor: (theme) => theme.palette.text.secondary,
  },
  '& .MuiChip-root.Mui-disabled': { opacity: 1 },
};

type RecordFormAttributeEditorContext = {
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
  migrationReviews?: Readonly<Record<string, MigrationFieldReview>>;
  resolvedValues: Readonly<Record<string, ResolvedValue>>;
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
  migrationReviews = {},
  onChange,
  onRecordUpdated,
  resolvedValues,
  value,
}: Props) => {
  const { t } = useTranslation();
  const { format: formatInstant } = useInstantFormat();
  const resolvedValue = resolvedValues[attribute.code];
  // The context a shown value comes from; a value without one is not inherited.
  const sourceContext = resolvedValue?.source_context;
  const inherited =
    !hasLocalFormValue(existingValues, attribute.code, contextId) &&
    sourceContext !== undefined &&
    sourceContext.id !== contextId;
  const defaultOnly = isDefaultContextOnly(
    attribute,
    contextId,
    defaultContextId,
  );
  const readonly = attribute.readonly === true;
  const lockedBy = lockedAttributes[attribute.code];
  const protectedValue = readonly || defaultOnly || lockedBy !== undefined;
  // Clearing a scalar's own value in a context lets it inherit again; an
  // empty relationship or file list would replace the inherited one instead.
  const canInherit =
    !disabled &&
    !protectedValue &&
    contextId !== defaultContextId &&
    attribute.context_fallback !== attributeContextFallbacks.none &&
    attribute.value_type !== attributeValueTypes.relationship &&
    attribute.value_type !== attributeValueTypes.file &&
    hasLocalFormValue(existingValues, attribute.code, contextId);
  const savedStatus = savedStatusState(
    attribute,
    statusSavedValues,
    contextId,
    statusParentContextIds,
  );

  const inheritedHelperText = () => {
    if (!inherited) return undefined;
    const context = sourceContext.code;
    const fromDefault = context === defaultContextCode;
    if (
      attribute.value_type === attributeValueTypes.relationship ||
      attribute.value_type === attributeValueTypes.file
    )
      return fromDefault
        ? t('records.inheritedFromDefaultContext')
        : t('records.inheritedFromContext', { context });
    const value =
      attribute.value_type === attributeValueTypes.datetime &&
      typeof resolvedValue.value === 'string'
        ? formatInstant(resolvedValue.value)
        : formatAttributeValue(attribute, resolvedValue.value);
    return fromDefault
      ? t('records.inheritedValueFromDefaultContext', { value })
      : t('records.inheritedValue', { context, value });
  };
  // Why the field cannot be changed here, or where its value comes from,
  // then what it holds and accepts.
  const helperText =
    [
      readonly
        ? attribute.generated && recordId === undefined
          ? t('records.assignedWhenSaved')
          : t('records.managedBySystem')
        : lockedBy !== undefined
          ? t('records.lockedByStatus', { status: lockedBy })
          : defaultOnly
            ? t('records.managedInDefault')
            : inheritedHelperText(),
      attributeHint(attribute, t),
    ]
      .filter(Boolean)
      .join(' ') || undefined;

  return (
    <>
      {inherited && attribute.value_type === attributeValueTypes.file && (
        <AttributeValue attribute={attribute} value={resolvedValue.value} />
      )}
      <Box sx={protectedValue ? protectedValueSx : { display: 'contents' }}>
        <RecordAttributeEditor
          attribute={attribute}
          component={component}
          // The server assigns a generated value on save, so the user has
          // nothing to fill even when the record schema requires it.
          required={required && !attribute.generated}
          contextId={contextId}
          disabled={disabled || protectedValue}
          reviewDisabled={disabled}
          statusBaseline={savedStatus.current}
          inheritedStatus={savedStatus.inherited}
          statusTransitions={statusTransitions.filter(
            (edge) => edge.attribute_code === attribute.code,
          )}
          recordId={recordId}
          files={filesForAttribute(existingValues, attribute.code, contextId)}
          error={fieldErrors[attribute.code]}
          helperText={helperText}
          migrationReview={migrationReviews[attribute.code]}
          onChange={onChange}
          onRecordUpdated={onRecordUpdated}
          value={value}
        />
      </Box>
      {canInherit && (
        <Box>
          <Button
            onClick={() => onChange('')}
            size="small"
            startIcon={<Undo2Icon />}
          >
            {t('records.useInheritedValue')}
          </Button>
        </Box>
      )}
    </>
  );
};
