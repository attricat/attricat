import { VIEW_EDIT_LAYOUT_SPACING } from '../../views/constants';
import { useForm, useStore } from '@tanstack/react-form';
import {
  Alert,
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  Link,
  Paper,
  Skeleton,
  Stack,
  Typography,
} from '@mui/material';
import { PencilIcon } from 'lucide-react';
import { lexiconText } from '../../lexicon/lexicon';
import type {
  Attribute,
  BlueprintWithAttributes,
  ComponentReference,
  FormAttributeValue,
  ViewDefinition,
} from '../api';
import { attributeLabel } from '../recordDisplay';
import {
  recordFormValidationMessages,
  missingRequiredAttributes,
  relationshipTargetsForForm,
  serializeAttributeValues,
  validateRecordForm,
  valuesForForm,
} from '../recordForm';
import {
  editableFormAttributes,
  recordSchemaRequiredAttributes,
  headingEditableAttributes,
  unplacedEditableAttributes,
  unplacedRequiredAttributes,
} from '../recordFormAttributes';
import {
  viewFieldEditComponents,
  viewFieldEditors,
} from '../../views/viewFieldComponents';
import { resolveEditComponent } from '../../views/components/registry';
import { EditableRecordLayout } from '../../views/components/EditableRecordLayout';
import { draftEditors, type DraftEditor } from '../../drafts/constants';
import { DraftRestoreDialog } from '../../drafts/DraftRestoreDialog';
import { formFieldsDraftSchema } from '../../drafts/schemas';
import { useEditorDraft } from '../../drafts/useEditorDraft';
import type { MigrationFieldReview } from '../migrationReview';
import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from 'react';
import { useTranslation } from 'react-i18next';
import { attributeValueTypes } from '../valueTypes';
import { RecordBlueprintSelect } from './RecordBlueprintSelect';
import {
  savedStatusState,
  statusConfiguration,
  statusLocks,
  statusTransitionDenial,
} from '../status';
import { useStatusTransitionDenialText } from '../useStatusTransitionDenialText';
import { checkViolationError } from '../../../api/checkViolations';
import { violationFieldErrors } from '../checkViolations';
import { useViolationText } from '../../../components/useViolationText';
import { ApiErrorAlert } from '../../../components/CheckViolationsAlert';
import { RecordFormAttributeEditor } from './RecordFormAttributeEditor';
import { useQueuedFileUploadsContext } from '../../files/queuedFileUploads';
import { useInitialFocus } from '../../../components/initialFocus';
import { FormActionBar } from '../../../components/FormActionBar';
import {
  FormErrorSummary,
  type FormErrorSummaryItem,
} from '../../../components/FormErrorSummary';
import { UnsavedChangesGuard } from '../../drafts/UnsavedChangesGuard';
import { FieldRevealContext, useFieldReveal } from '../../views/fieldReveal';
import { viewPlacedFields } from '../../views/viewFieldComponents';
import { migrationReviewSelector } from '../constants';

export type RecordFormHandle = {
  /** Removes the persisted draft after a confirmed save. */
  clearDraft: () => void;
};

/** Identifies where unsaved field values are kept across refreshes. */
export type RecordFormDraft = {
  editor: DraftEditor;
  resource: readonly (string | number)[];
  /** Marker for the loaded source beyond the initial field values. */
  source: string;
};

const draftFieldSeparator = '\n';
/** Padding of the form's surface, which the action bar spans. */
const formPadding = 3;
const submitShortcut = 'Control+Enter Meta+Enter';

const pickDraftFields = (
  fields: Record<string, string>,
  codes: readonly string[],
) =>
  Object.fromEntries(
    codes.flatMap((code) =>
      fields[code] === undefined ? [] : [[code, fields[code]]],
    ),
  );

// Creating and migrating a record have no saved status edges, parent
// contexts or resolved values to show.
const noStatusParentContextIds: readonly string[] = [];
const noStatusTransitions: readonly [] = [];
const noResolvedValues = {};
const noPendingConfirmations: readonly FormErrorSummaryItem[] = [];
const fieldSkeletons = ['first', 'second', 'third'];

/**
 * A field the user changed carries an `action.changed` bar at its leading
 * edge, outside the control; the action bar counts them, so color is never
 * the only sign.
 */
const changedFieldSx = {
  position: 'relative',
  '&[data-changed]::before': {
    bgcolor: 'action.changed',
    borderRadius: 0.5,
    bottom: 6,
    content: '""',
    left: -12,
    position: 'absolute',
    top: 6,
    width: 3,
  },
} as const;

type RecordFormProps = {
  expectedUpdatedAt?: string;
  blueprint?: BlueprintWithAttributes;
  draft?: RecordFormDraft;
  initialValues?: ReturnType<typeof valuesForForm>;
  contextId?: string | null;
  defaultContextId?: string | null;
  existingValues?: FormAttributeValue[];
  isLoadingBlueprint?: boolean;
  showAllAttributes?: boolean;
  /** Lays out every editable field instead of the blueprint's detail view. */
  layout?: ViewDefinition;
  migrationReviews?: Readonly<Record<string, MigrationFieldReview>>;
  /**
   * Choices made outside the fields that must be confirmed before submitting.
   * Each `id` matches a `data-field-code` element in `children` to go to.
   */
  pendingConfirmations?: readonly FormErrorSummaryItem[];
  /** Shown above the fields, inside the form. */
  children?: ReactNode;
  requiredAttributes?: readonly string[];
  /** Load or save failure; check violations are placed on their fields. */
  error?: Error | null;
  onLoadBlueprint?: (code: string, version?: number) => void;
  lockedBlueprint?: boolean;
  onCancel?: () => void;
  /** Lets the user go back to choosing a blueprint. */
  onChangeBlueprint?: () => void;
  /** Offers a second submit that keeps the form open for the next record. */
  addAnotherLabel?: string;
  /** Asks before an in-app navigation drops entered values; needs a router. */
  guardUnsavedChanges?: boolean;
  onSubmit: (input: {
    expected_updated_at?: string;
    values: ReturnType<typeof serializeAttributeValues>;
    relationships: ReturnType<typeof relationshipTargetsForForm>;
    addAnother: boolean;
  }) => void;
  submitLabel: string;
};

/**
 * Enters a new record's values, or the values of a record being migrated to
 * another blueprint version. Existing records are edited inline instead.
 */
export const RecordForm = forwardRef<RecordFormHandle, RecordFormProps>(
  (
    {
      blueprint,
      expectedUpdatedAt,
      draft: draftOptions,
      initialValues = {},
      contextId = null,
      defaultContextId = null,
      existingValues = [],
      isLoadingBlueprint = false,
      showAllAttributes: showAllProp = false,
      layout,
      migrationReviews = {},
      pendingConfirmations = noPendingConfirmations,
      children,
      requiredAttributes = [],
      error,
      onLoadBlueprint,
      lockedBlueprint = false,
      onCancel,
      onChangeBlueprint,
      addAnotherLabel,
      guardUnsavedChanges = false,
      onSubmit,
      submitLabel,
    },
    ref,
  ) => {
    const { t } = useTranslation();
    const violationText = useViolationText();
    const denialText = useStatusTransitionDenialText();
    // A versioned editing session keeps values and its concurrency token from
    // the same snapshot. Refetches must not silently rebase unsaved edits.
    const [baseline] = useState({
      fields: initialValues,
      values: existingValues,
      version: expectedUpdatedAt,
    });
    const initialFields =
      expectedUpdatedAt === undefined ? initialValues : baseline.fields;
    const savedValues = baseline.values;
    const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});
    const [formError, setFormError] = useState<string>();
    // Records are entered with the detail layout; each display component
    // edits with its paired edit component.
    const detailView = blueprint?.blueprint.views.detail;
    // A given layout places every field itself, like showing them all.
    const showAllAttributes = showAllProp || layout !== undefined;
    const shownView = layout ?? (showAllAttributes ? undefined : detailView);
    const fieldComponents = viewFieldEditComponents(detailView);
    // Required by the caller or, in the default context, by the record schema.
    const requiredCodes = new Set([
      ...requiredAttributes,
      ...(blueprint && contextId === defaultContextId
        ? recordSchemaRequiredAttributes(blueprint.blueprint.record_schema)
        : []),
    ]);
    const editableAttributes = editableFormAttributes(
      blueprint?.attributes ?? [],
      {
        contextId,
        defaultContextId,
        requiredCodes,
        usesFormLayout: Boolean(blueprint && !showAllAttributes && !detailView),
      },
    );
    const fieldEditors = viewFieldEditors(fieldComponents, editableAttributes);
    // Files queued while creating a record are submitted with it, so they
    // count toward required file attributes.
    const queuedFiles = useQueuedFileUploadsContext()?.pending;
    const fileCounts = useMemo(
      () =>
        queuedFiles &&
        Object.fromEntries(
          Object.entries(queuedFiles).map(([code, items]) => [
            code,
            items.length,
          ]),
        ),
      [queuedFiles],
    );
    const checks = checkViolationError(error);
    // A field's server violation is cleared once the user edits that field.
    const [editedAfterError, setEditedAfterError] = useState<{
      error: unknown;
      codes: readonly string[];
    }>({ error: undefined, codes: [] });
    const editedCodes =
      editedAfterError.error === error ? editedAfterError.codes : [];
    const serverViolations = checks
      ? violationFieldErrors(
          checks.violations,
          editableAttributes.map((attribute) => attribute.code),
          violationText,
        )
      : undefined;
    const serverFieldErrors = Object.fromEntries(
      Object.entries(serverViolations?.fieldErrors ?? {}).filter(
        ([code]) => !editedCodes.includes(code),
      ),
    );
    const markEdited = (code: string) => {
      if (!checks || editedCodes.includes(code)) return;
      setEditedAfterError((current) => ({
        error,
        codes: current.error === error ? [...current.codes, code] : [code],
      }));
    };
    const lockedAttributes = statusLocks(
      blueprint?.attributes ?? [],
      savedValues,
      contextId,
      noStatusParentContextIds,
    );
    // Locked values are never resubmitted; the server rejects any change.
    const submittedAttributes = editableAttributes.filter(
      (attribute) => lockedAttributes[attribute.code] === undefined,
    );
    // The heading only displays values, so its fields are edited first.
    const headingFields =
      detailView && !showAllAttributes
        ? headingEditableAttributes(editableAttributes, detailView)
        : [];
    // Editable fields the layout leaves out.
    const otherAttributes =
      blueprint && !showAllAttributes
        ? [
            ...new Set([
              ...(detailView
                ? unplacedEditableAttributes(editableAttributes, detailView)
                : []),
              ...unplacedRequiredAttributes(editableAttributes, detailView, [
                ...requiredCodes,
              ]),
            ]),
          ]
        : [];
    // Fields in the order the form shows them, to go to the first error.
    const fieldOrder = [
      ...new Set([
        ...headingFields.map((attribute) => attribute.code),
        ...viewPlacedFields(shownView),
        ...otherAttributes.map((attribute) => attribute.code),
        ...editableAttributes.map((attribute) => attribute.code),
      ]),
    ];
    // The blueprint picker comes first; once a blueprint loads, its first
    // field, or in a migration the first field to review. A failed save
    // leaves focus where it is.
    const formRef = useInitialFocus<HTMLFormElement>(
      isLoadingBlueprint
        ? null
        : blueprint
          ? `${blueprint.blueprint.id}:${blueprint.blueprint.version}`
          : lockedBlueprint
            ? null
            : '',
      migrationReviewSelector,
    );
    // Opens the tab or accordion holding a field and focuses it.
    const { request: revealRequest, reveal: revealField } =
      useFieldReveal(formRef);
    const errorsRef = useRef<HTMLDivElement>(null);
    const focusErrors = () => {
      errorsRef.current?.focus({ preventScroll: true });
      errorsRef.current?.scrollIntoView?.({ block: 'nearest' });
    };
    const validateFields = (fields: Record<string, string>) => {
      if (!blueprint) return { fieldErrors: {} };
      const validation = validateRecordForm(
        editableAttributes,
        fields,
        // Schema-required fields read as required rather than as a schema
        // failure once they are cleared.
        [...requiredCodes],
        contextId === defaultContextId
          ? blueprint.blueprint.record_schema
          : undefined,
        {
          ...recordFormValidationMessages(),
          // A migration fills what the target schema requires; a new record
          // only needs a value.
          ...(expectedUpdatedAt === undefined && {
            required: t('records.valueRequired'),
          }),
        },
        fieldEditors,
        fileCounts,
      );
      for (const attribute of editableAttributes) {
        const config = statusConfiguration(attribute);
        if (!config) continue;
        const saved = savedStatusState(
          attribute,
          savedValues,
          contextId,
          noStatusParentContextIds,
        );
        const denial = statusTransitionDenial(config, {
          attributeCode: attribute.code,
          baseline: saved.current,
          inherited: saved.inherited,
          selected: fields[attribute.code] ?? '',
          transitions: noStatusTransitions,
        });
        if (denial) validation.fieldErrors[attribute.code] = denialText(denial);
      }
      setFieldErrors(validation.fieldErrors);
      setFormError(validation.formError);
      return validation;
    };
    // The submit button that was used, read when the form submits.
    const addAnother = useRef(false);
    const [submitAttempted, setSubmitAttempted] = useState(false);
    const [confirmingBlueprintChange, setConfirmingBlueprintChange] =
      useState(false);
    const changeConfirmed = useRef(false);
    const form = useForm({
      defaultValues: {
        blueprintCode: blueprint?.blueprint.code ?? '',
        fields: initialFields,
      },
      onSubmit: ({ value }) => {
        if (isLoadingBlueprint) return;
        if (!blueprint) {
          onLoadBlueprint?.(value.blueprintCode);
          return;
        }
        setSubmitAttempted(true);
        const validation = validateFields(value.fields);
        if (
          pendingConfirmations.length > 0 ||
          Object.keys(validation.fieldErrors).length > 0 ||
          validation.formError
        ) {
          // Confirmations are shown above the fields, so they come first.
          const first =
            pendingConfirmations[0]?.id ??
            fieldOrder.find(
              (code) => validation.fieldErrors[code] !== undefined,
            );
          if (first) revealField(first);
          else focusErrors();
          return;
        }
        onSubmit({
          addAnother: addAnother.current,
          ...(baseline.version
            ? { expected_updated_at: baseline.version }
            : {}),
          values: serializeAttributeValues(
            submittedAttributes,
            value.fields,
            contextId,
            fieldEditors,
          ),
          relationships: relationshipTargetsForForm(
            submittedAttributes,
            value.fields,
            contextId,
          ),
        });
      },
    });

    // Files upload separately and are never kept in a draft.
    const draftCodes = editableAttributes
      .filter((attribute) => attribute.value_type !== attributeValueTypes.file)
      .map((attribute) => attribute.code)
      .join(draftFieldSeparator);
    const fields = useStore(form.store, (state) => state.values.fields);
    const draftFields = useMemo(
      () => pickDraftFields(fields, draftCodes.split(draftFieldSeparator)),
      [draftCodes, fields],
    );
    const initialDraftFields = JSON.stringify(
      pickDraftFields(initialFields, draftCodes.split(draftFieldSeparator)),
    );
    const fieldsChanged = JSON.stringify(draftFields) !== initialDraftFields;
    const draft = useEditorDraft({
      dirty: fieldsChanged,
      // Unused without draft options; `ready` keeps the draft off.
      editor: draftOptions?.editor ?? draftEditors.recordCreate,
      ready: Boolean(draftOptions && blueprint),
      resource: draftOptions?.resource ?? [],
      schema: formFieldsDraftSchema,
      source: JSON.stringify([draftOptions?.source, initialDraftFields]),
      value: draftFields,
    });

    const restoreDraft = () => {
      const restored = draft.restore();
      if (!restored) return;
      const nextFields = {
        ...form.state.values.fields,
        ...pickDraftFields(restored, draftCodes.split(draftFieldSeparator)),
      };
      validateFields(nextFields);
      form.setFieldValue('fields', nextFields);
    };

    useImperativeHandle(ref, () => ({ clearDraft: draft.clear }));

    const missingRequired = missingRequiredAttributes(
      submittedAttributes,
      fields,
      requiredCodes,
      fileCounts,
    );
    const allFieldErrors = Object.fromEntries(
      Object.entries({ ...serverFieldErrors, ...fieldErrors }).filter(
        // Queuing a file fills a required file field without a field change.
        ([code]) => !((fileCounts?.[code] ?? 0) > 0),
      ),
    );
    const attributesByCode = new Map(
      editableAttributes.map((attribute) => [attribute.code, attribute]),
    );
    const errorItems = [
      ...pendingConfirmations,
      ...fieldOrder.flatMap((code) => {
        const message = allFieldErrors[code];
        const attribute = attributesByCode.get(code);
        return message && attribute
          ? [{ id: code, label: attributeLabel(attribute), message }]
          : [];
      }),
    ];
    // A save the server rejected goes to its first misplaced field, or to the
    // errors above the form. An error present on mount, such as a failed
    // blueprint load, is only shown.
    const handledError = useRef(error);
    useEffect(() => {
      if (!error || error === handledError.current) return;
      handledError.current = error;
      const first = fieldOrder.find(
        (code) => serverFieldErrors[code] !== undefined,
      );
      if (first) revealField(first);
      else focusErrors();
    });

    const hasQueuedFiles = Object.values(fileCounts ?? {}).some(
      (count) => count > 0,
    );
    // Fields whose value differs from the one the form started with.
    const changedCodes = new Set(
      editableAttributes
        .filter((attribute) =>
          attribute.value_type === attributeValueTypes.file
            ? (fileCounts?.[attribute.code] ?? 0) > 0
            : (fields[attribute.code] ?? '') !==
              (initialFields[attribute.code] ?? ''),
        )
        .map((attribute) => attribute.code),
    );
    const editorContext = {
      statusParentContextIds: noStatusParentContextIds,
      disabled: isLoadingBlueprint,
      contextId,
      defaultContextId,
      existingValues,
      statusSavedValues: savedValues,
      lockedAttributes,
      statusTransitions: noStatusTransitions,
      fieldErrors: allFieldErrors,
      migrationReviews,
      resolvedValues: noResolvedValues,
    };
    const submit = (another: boolean) => {
      addAnother.current = another;
      void form.handleSubmit();
    };
    const status =
      isLoadingBlueprint || !blueprint ? undefined : submitAttempted &&
        errorItems.length > 0 ? (
        <Link
          color="error"
          component="button"
          onClick={() => revealField(errorItems[0].id)}
          type="button"
          variant="body2"
        >
          {t('records.fieldsNeedAttention', { count: errorItems.length })}
        </Link>
      ) : missingRequired.length > 0 ? (
        <Link
          color="text.secondary"
          component="button"
          onClick={() => {
            const missing = new Set(
              missingRequired.map((attribute) => attribute.code),
            );
            const first = fieldOrder.find((code) => missing.has(code));
            if (first) revealField(first);
          }}
          type="button"
          variant="body2"
        >
          {t('records.requiredFieldsLeft', { count: missingRequired.length })}
        </Link>
      ) : pendingConfirmations.length > 0 ? (
        <Link
          color="text.secondary"
          component="button"
          onClick={() => revealField(pendingConfirmations[0].id)}
          type="button"
          variant="body2"
        >
          {t('records.changesToConfirm', {
            count: pendingConfirmations.length,
          })}
        </Link>
      ) : undefined;

    return (
      <Paper
        component="form"
        noValidate
        ref={formRef}
        onKeyDown={(event: KeyboardEvent<HTMLFormElement>) => {
          if (event.key !== 'Enter' || !(event.ctrlKey || event.metaKey))
            return;
          event.preventDefault();
          if (!isLoadingBlueprint) submit(false);
        }}
        onSubmit={(event) => {
          event.preventDefault();
          submit(false);
        }}
        sx={{ mt: 4, p: formPadding }}
      >
        <Dialog
          // Changing removes the button that opened it; focus moves to the
          // blueprint picker instead.
          disableRestoreFocus
          onClose={() => setConfirmingBlueprintChange(false)}
          open={confirmingBlueprintChange}
          slotProps={{
            transition: {
              // The page is hidden from assistive technology until the
              // dialog has closed, so the picker can only take focus then.
              onExited: () => {
                if (!changeConfirmed.current) return;
                changeConfirmed.current = false;
                onChangeBlueprint?.();
              },
            },
          }}
        >
          <DialogTitle>{t('records.changeBlueprintTitle')}</DialogTitle>
          <DialogContent>
            <DialogContentText>
              {t('records.changeBlueprintDescription')}
            </DialogContentText>
          </DialogContent>
          <DialogActions>
            <Button onClick={() => setConfirmingBlueprintChange(false)}>
              {t('drafts.keepEditing')}
            </Button>
            <Button
              color="error"
              onClick={() => {
                changeConfirmed.current = true;
                setConfirmingBlueprintChange(false);
                draft.clear();
              }}
              variant="contained"
            >
              {t('records.changeBlueprintConfirm')}
            </Button>
          </DialogActions>
        </Dialog>
        <DraftRestoreDialog
          draft={draft.pending}
          onDiscard={draft.discard}
          onRestore={restoreDraft}
        />
        {guardUnsavedChanges &&
          !isLoadingBlueprint &&
          (fieldsChanged || hasQueuedFiles) && (
            <UnsavedChangesGuard
              description={t('records.unsavedFormDescription')}
              onDiscard={draft.clear}
              title={t('records.unsavedFormTitle')}
            />
          )}
        <Stack spacing={VIEW_EDIT_LAYOUT_SPACING}>
          {(error ||
            formError ||
            (submitAttempted && errorItems.length > 0)) && (
            <Stack
              ref={errorsRef}
              spacing={2}
              sx={{ outline: 'none' }}
              tabIndex={-1}
            >
              {submitAttempted && errorItems.length > 0 && (
                <FormErrorSummary
                  errors={errorItems}
                  onSelect={revealField}
                  title={t('records.fieldsNeedAttention', {
                    count: errorItems.length,
                  })}
                />
              )}
              {error && (
                <ApiErrorAlert
                  error={error}
                  violations={serverViolations?.unplaced}
                />
              )}
              {formError && <Alert severity="error">{formError}</Alert>}
            </Stack>
          )}
          {children}
          {!blueprint && !lockedBlueprint && (
            <form.Field name="blueprintCode">
              {(field) => (
                <RecordBlueprintSelect
                  onChange={field.handleChange}
                  value={field.state.value}
                />
              )}
            </form.Field>
          )}
          {blueprint && onChangeBlueprint && !lockedBlueprint && (
            <Box sx={{ alignItems: 'center', display: 'flex', gap: 1 }}>
              <Typography>
                {t('records.blueprintChosen', {
                  blueprint: lexiconText(blueprint.blueprint.name),
                })}
              </Typography>
              <Button
                disabled={isLoadingBlueprint}
                onClick={() =>
                  fieldsChanged || hasQueuedFiles
                    ? setConfirmingBlueprintChange(true)
                    : onChangeBlueprint()
                }
                size="small"
                startIcon={<PencilIcon />}
              >
                {t('records.changeBlueprint')}
              </Button>
            </Box>
          )}
          {!blueprint && isLoadingBlueprint && (
            // Holds the form's place while its fields load.
            <Stack aria-busy spacing={VIEW_EDIT_LAYOUT_SPACING}>
              {fieldSkeletons.map((key) => (
                <Skeleton height={40} key={key} variant="rounded" />
              ))}
            </Stack>
          )}
          {blueprint && (
            <form.Field name="fields">
              {(field) => {
                const renderEditor = (
                  attribute: Attribute,
                  component?: ComponentReference | null,
                ) => (
                  // Marks where the field is, to focus it from an error, and
                  // that the user changed it.
                  <Box
                    data-changed={changedCodes.has(attribute.code) || undefined}
                    data-field-code={attribute.code}
                    sx={changedFieldSx}
                  >
                    <RecordFormAttributeEditor
                      {...editorContext}
                      attribute={attribute}
                      component={
                        resolveEditComponent(component) ??
                        fieldComponents.get(attribute.code)
                      }
                      required={requiredCodes.has(attribute.code)}
                      onChange={(nextValue) => {
                        const nextFields = {
                          ...field.state.value,
                          [attribute.code]: nextValue,
                        };
                        markEdited(attribute.code);
                        validateFields(nextFields);
                        field.handleChange(nextFields);
                      }}
                      value={field.state.value[attribute.code] ?? ''}
                    />
                  </Box>
                );
                return (
                  <FieldRevealContext value={revealRequest}>
                    <EditableRecordLayout
                      attributes={blueprint.attributes}
                      fallbackVisibilityScope={
                        showAllAttributes
                          ? undefined
                          : detailView
                            ? 'detail'
                            : 'form'
                      }
                      headingAttributes={headingFields}
                      invalidFields={new Set(Object.keys(allFieldErrors))}
                      otherAttributes={otherAttributes}
                      renderEditor={renderEditor}
                      values={noResolvedValues}
                      view={shownView}
                    />
                  </FieldRevealContext>
                );
              }}
            </form.Field>
          )}
        </Stack>
        <FormActionBar inset={formPadding} status={status}>
          {changedCodes.size > 0 && (
            <Typography
              color="text.secondary"
              sx={{ alignSelf: 'center' }}
              variant="body2"
            >
              {t('records.fieldsChanged', { count: changedCodes.size })}
            </Typography>
          )}
          {onCancel && (
            <Button disabled={isLoadingBlueprint} onClick={onCancel}>
              {t('common.cancel')}
            </Button>
          )}
          {blueprint && addAnotherLabel && (
            <Button
              disabled={isLoadingBlueprint}
              onClick={() => submit(true)}
              variant="outlined"
            >
              {addAnotherLabel}
            </Button>
          )}
          <Button
            aria-keyshortcuts={submitShortcut}
            disabled={isLoadingBlueprint}
            type="submit"
            variant="contained"
          >
            {isLoadingBlueprint ? t('records.loadingBlueprint') : submitLabel}
          </Button>
        </FormActionBar>
      </Paper>
    );
  },
);
