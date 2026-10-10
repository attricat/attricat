import { useQueryClient } from '@tanstack/react-query';
import { Alert, Box, Button, Stack, Typography } from '@mui/material';
import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useRef,
  type ReactNode,
} from 'react';
import { useTranslation } from 'react-i18next';
import { RotateCcwIcon } from 'lucide-react';
import { checkViolationError } from '../../../api/checkViolations';
import { ApiErrorAlert } from '../../../components/CheckViolationsAlert';
import { useViolationText } from '../../../components/useViolationText';
import { useBeforeUnloadWarning } from '../../drafts/useBeforeUnloadWarning';
import { principalConfiguration } from '../../principals/principal';
import {
  EditableRecordLayout,
  EditableRecordSection,
} from '../../views/components/EditableRecordLayout';
import { RecordView } from '../../views/components/RecordView';
import type { ResolvedValue } from '../../views/components/ValueField';
import {
  resolveEditComponent,
  resolveViewComponent,
} from '../../views/components/registry';
import {
  viewFieldEditComponents,
  viewFieldEditors,
} from '../../views/viewFieldComponents';
import {
  type Attribute,
  type ComponentReference,
  type RecordFormResponse,
  type StatusTransitionAccess,
} from '../api';
import { violationFieldErrors } from '../checkViolations';
import {
  recordFormValidationMessages,
  validateRecordForm,
  valuesForForm,
} from '../recordForm';
import {
  editableFormAttributes,
  recordSchemaRequiredAttributes,
  headingEditableAttributes,
  smartFillFormFields,
  unplacedEditableAttributes,
  type ResolvedFormValues,
} from '../recordFormAttributes';
import { schemaMismatchField } from '../recordFieldSaves';
import { recordFormOptions } from '../queryOptions';
import { statusConfiguration, statusLocks } from '../status';
import {
  useRecordFieldSaves,
  type ConflictResolution,
} from '../useRecordFieldSaves';
import { attributeValueTypes } from '../valueTypes';
import { RecordFormAttributeEditor } from './RecordFormAttributeEditor';
import { FieldValueAtRest } from './FieldValueAtRest';
import { InlineFieldEditor } from './InlineFieldEditor';
import { ReusableAttributeAttachControl } from './ReusableAttributeAttachControl';
import { UnsavedFieldChangesGuard } from './UnsavedFieldChangesGuard';

type Props = {
  recordId: string;
  form: RecordFormResponse;
  attributes: readonly Attribute[];
  contextId: string | null;
  defaultContextId: string | null;
  statusParentContextIds: readonly string[];
  statusTransitions?: readonly StatusTransitionAccess[];
  resolvedValues: Record<string, ResolvedValue>;
  reusableResolvedValues: Record<string, ResolvedValue>;
  view: RecordFormResponse['blueprint']['blueprint']['views']['detail'];
  renderAttributeDecoration?: (attribute: Attribute) => ReactNode;
  renderAttributePanel?: (attribute: Attribute) => ReactNode;
  renderFilePanel?: (attribute: Attribute, fileId: string) => ReactNode;
  /** Reports whether changes are waiting to be saved in this context. */
  onPendingChange?: (pending: boolean) => void;
  /** Lays the view out in one column, as in a narrow panel. */
  singleColumn?: boolean;
};

export type RecordInlineFieldsHandle = {
  /** Current field values, including changes that are not saved yet. */
  getDraftValues: () => Record<string, string>;
  /** Saves Smart Fill suggestions for editable fields as one change. */
  applySmartFillValues: (values: Record<string, string>) => void;
};

/** Changes of these kinds are final choices rather than typing. */
const commitsImmediately = (attribute: Attribute) =>
  attribute.value_type === attributeValueTypes.relationship ||
  attribute.value_type === attributeValueTypes.boolean ||
  attribute.value_type === attributeValueTypes.file ||
  statusConfiguration(attribute) !== undefined ||
  principalConfiguration(attribute) !== undefined;

/**
 * The record's detail view with every field the user may change rendered as
 * an always-editable control. Each committed field saves on its own.
 */
export const RecordInlineFields = forwardRef<RecordInlineFieldsHandle, Props>(
  (
    {
      recordId,
      form,
      attributes,
      contextId,
      defaultContextId,
      statusParentContextIds,
      statusTransitions = [],
      resolvedValues,
      reusableResolvedValues,
      view,
      renderAttributeDecoration,
      renderAttributePanel,
      renderFilePanel,
      onPendingChange,
      singleColumn,
    },
    ref,
  ) => {
    const { t } = useTranslation();
    const client = useQueryClient();
    const violationText = useViolationText();
    const allAttributes = [...attributes, ...form.reusable_attributes];
    const existingValues = [...form.values, ...form.reusable_values];
    const editable = form.can_write
      ? editableFormAttributes(allAttributes, {
          contextId,
          defaultContextId,
        })
      : [];
    const locks = statusLocks(
      allAttributes,
      existingValues,
      contextId,
      statusParentContextIds,
    );
    const changeable = editable.filter(
      (attribute) => locks[attribute.code] === undefined,
    );
    const changeableCodes = changeable.map((attribute) => attribute.code);
    const editComponents = viewFieldEditComponents(view);
    const fieldRules = viewFieldEditors(editComponents, changeable);
    // The record schema only applies to values in the default context.
    const requiredAttributes =
      contextId === defaultContextId
        ? recordSchemaRequiredAttributes(form.blueprint.blueprint.record_schema)
        : [];
    const saves = useRecordFieldSaves({
      recordId,
      contextId,
      attributes: changeable,
      savedFields: valuesForForm(allAttributes, existingValues, contextId),
      updatedAt: form.record.updated_at,
      fieldRules,
      onPendingChange,
    });
    const hasPending = Object.keys(saves.pending).length > 0;
    // Pending changes left after a failed or conflicting save; leaving the
    // record drops them. Changes still being saved finish on their own.
    const unsaved = hasPending && !saves.saving;
    useImperativeHandle(ref, () => ({
      getDraftValues: () => ({ ...saves.fields }),
      applySmartFillValues: (values) => {
        const fields = smartFillFormFields(changeable, values);
        if (Object.keys(fields).length > 0) saves.commitMany(fields);
      },
    }));
    // Leaving would drop changes that are not saved yet.
    useBeforeUnloadWarning(hasPending || saves.saving);
    // Changes dropped with these fields, for example on opening another
    // record, no longer wait to be saved.
    const reportPending = useRef(onPendingChange);
    useEffect(() => {
      reportPending.current = onPendingChange;
    });
    useEffect(() => () => reportPending.current?.(false), []);

    const violations = checkViolationError(saves.error);
    const placedViolations = violations
      ? violationFieldErrors(
          violations.violations,
          changeableCodes,
          violationText,
        )
      : undefined;
    const schemaField = schemaMismatchField(saves.error, changeableCodes);
    const serverErrors: Record<string, string> = {
      ...placedViolations?.fieldErrors,
      ...(schemaField && {
        [schemaField.code]: schemaField.missing
          ? t('records.valueRequired')
          : t('records.schemaValidationFailed'),
      }),
    };
    // A change only reads as not saved once a save failed; marking fields
    // while a save is in flight would shift the layout under the pointer.
    const fieldErrors = saves.error
      ? Object.fromEntries(
          Object.keys(saves.pending).map((code) => [
            code,
            serverErrors[code] ?? t('records.changeNotSaved'),
          ]),
        )
      : {};
    // Errors no field can show, and violations of fields not on the page.
    const generalError =
      saves.error &&
      !saves.conflict &&
      (Object.keys(serverErrors).length === 0 ||
        (placedViolations?.unplaced.length ?? 0) > 0)
        ? saves.error
        : undefined;

    const resolveConflict = async (resolution: ConflictResolution) => {
      const latest = await client.fetchQuery({
        ...recordFormOptions(recordId),
        staleTime: 0,
      });
      saves.resolveConflict(resolution, {
        savedFields: valuesForForm(
          [...latest.blueprint.attributes, ...latest.reusable_attributes],
          [...latest.values, ...latest.reusable_values],
          contextId,
        ),
        updatedAt: latest.record.updated_at,
      });
    };

    const renderEditor = (
      attribute: Attribute,
      component?: ComponentReference | null,
    ) => {
      if (!changeableCodes.includes(attribute.code)) return null;
      const required = requiredAttributes.includes(attribute.code);
      const editComponent =
        resolveEditComponent(component) ?? editComponents.get(attribute.code);
      const rules = fieldRules.get(attribute.code);
      const display = resolveViewComponent(component);
      const KeptDisplay = display?.showsWhileEditing
        ? display.valueRenderer
        : undefined;
      // A field whose save failed keeps its editor open to show the error.
      const AtRestDisplay =
        resolveViewComponent(editComponent)?.editsOnRequest &&
        fieldErrors[attribute.code] === undefined
          ? display?.valueRenderer
          : undefined;
      const editor = (
        <InlineFieldEditor
          immediate={commitsImmediately(attribute)}
          onCommit={(value) => saves.commit(attribute.code, value)}
          onRevert={() => saves.revert(attribute.code)}
          renderAtRest={
            AtRestDisplay &&
            (({ value, onEdit }) => (
              <FieldValueAtRest
                attribute={attribute}
                onEdit={onEdit}
                required={required}
              >
                <AtRestDisplay
                  attribute={attribute}
                  component={component}
                  contextId={contextId ?? undefined}
                  recordId={recordId}
                  value={value}
                />
              </FieldValueAtRest>
            ))
          }
          validate={(value) =>
            validateRecordForm(
              [attribute],
              { [attribute.code]: value },
              requiredAttributes,
              undefined,
              {
                ...recordFormValidationMessages(),
                // Matches a required value the server rejects.
                required: t('records.valueRequired'),
              },
              rules ? new Map([[attribute.code, rules]]) : undefined,
            ).fieldErrors[attribute.code]
          }
          value={saves.fields[attribute.code] ?? ''}
        >
          {({ value, error, onChange }) => (
            <RecordFormAttributeEditor
              attribute={attribute}
              component={editComponent}
              contextId={contextId}
              defaultContextId={defaultContextId}
              recordId={recordId}
              existingValues={existingValues}
              fieldErrors={error ? { [attribute.code]: error } : fieldErrors}
              highlightedAttributes={[]}
              lockedAttributes={locks}
              migrationReviewMessages={{}}
              onChange={onChange}
              onRecordUpdated={saves.noteRecordUpdated}
              required={required}
              resolvedValues={resolvedValues as ResolvedFormValues}
              statusParentContextIds={statusParentContextIds}
              statusTransitions={statusTransitions}
              value={value}
            />
          )}
        </InlineFieldEditor>
      );
      return KeptDisplay ? (
        // Leaves room for the editor's label, which rises above its outline.
        <Stack spacing={4} useFlexGap>
          {/* Sits closer to the field above than the layout's gap so it
              reads as part of its own editor. */}
          <Box sx={{ mt: -4 }}>
            <KeptDisplay
              attribute={attribute}
              component={component}
              contextId={contextId ?? undefined}
              recordId={recordId}
              value={resolvedValues[attribute.code]?.value}
            />
          </Box>
          {editor}
        </Stack>
      ) : (
        editor
      );
    };
    const viewProps = {
      contextId: contextId ?? undefined,
      recordId,
      // Tabs and accordions point to a field whose change was not saved.
      invalidFields: new Set(Object.keys(fieldErrors)),
      renderAttributeDecoration,
      renderAttributePanel,
      renderFilePanel,
      renderEditor,
      singleColumn,
    };
    const blueprintChangeable = changeable.filter((attribute) =>
      attributes.some((placed) => placed.code === attribute.code),
    );

    return (
      <>
        {unsaved && <UnsavedFieldChangesGuard />}
        {saves.conflict && (
          <Alert
            action={
              <Box sx={{ display: 'flex', gap: 1 }}>
                <Button
                  color="inherit"
                  onClick={() => void resolveConflict('keepMine')}
                  size="small"
                >
                  {t('records.keepMyChanges')}
                </Button>
                <Button
                  color="inherit"
                  onClick={() => void resolveConflict('useTheirs')}
                  size="small"
                >
                  {t('records.useLatestValues')}
                </Button>
              </Box>
            }
            severity="warning"
            sx={{ mb: 2 }}
          >
            {t('records.changedByAnotherEditor')}
          </Alert>
        )}
        {generalError && (
          <ApiErrorAlert
            action={
              unsaved && (
                <Button
                  color="inherit"
                  onClick={saves.retry}
                  size="small"
                  startIcon={<RotateCcwIcon />}
                >
                  {t('errors.retry')}
                </Button>
              )
            }
            error={generalError}
            violations={placedViolations?.unplaced}
            sx={{ mb: 2 }}
          />
        )}
        <Typography
          aria-live="polite"
          color="text.secondary"
          sx={{ display: 'block', minHeight: '1.5em', mb: 1 }}
          variant="caption"
        >
          {saves.saving
            ? t('records.savingChanges')
            : hasPending
              ? t('records.unsavedChangeCount', {
                  count: Object.keys(saves.pending).length,
                })
              : null}
        </Typography>
        <EditableRecordLayout
          {...viewProps}
          attributes={attributes}
          fallbackVisibilityScope="detail"
          headingAttributes={headingEditableAttributes(
            blueprintChangeable,
            view,
          )}
          otherAttributes={unplacedEditableAttributes(
            blueprintChangeable,
            view,
          )}
          values={resolvedValues}
          view={view}
        />
        {form.reusable_attributes.length > 0 && (
          <EditableRecordSection
            headingLevel="h2"
            title={t('records.additionalAttributes')}
          >
            <RecordView
              {...viewProps}
              attributes={form.reusable_attributes}
              values={reusableResolvedValues}
            />
          </EditableRecordSection>
        )}
        {form.can_write && (
          <Box sx={{ mt: 3 }}>
            {/* Attaching changes the record; wait for field saves first. */}
            <ReusableAttributeAttachControl
              disabled={hasPending}
              recordId={recordId}
            />
          </Box>
        )}
      </>
    );
  },
);
