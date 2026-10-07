import { VIEW_EDIT_LAYOUT_SPACING } from '../../views/constants';
import { useForm, useStore } from '@tanstack/react-form';
import { Alert, Box, Button, Paper, Stack, Typography } from '@mui/material';
import type {
  Attribute,
  BlueprintWithAttributes,
  ComponentReference,
  FormAttributeValue,
} from '../api';
import {
  relationshipTargetsForForm,
  serializeAttributeValues,
  validateEntityForm,
  valuesForForm,
} from '../entityForm';
import {
  editableFormAttributes,
  entitySchemaRequiredAttributes,
  headingEditableAttributes,
  unplacedEditableAttributes,
  unplacedRequiredAttributes,
} from '../entityFormAttributes';
import {
  viewFieldEditComponents,
  viewFieldEditors,
} from '../../views/viewFieldComponents';
import { resolveEditComponent } from '../../views/components/registry';
import { EditableEntityLayout } from '../../views/components/EditableEntityLayout';
import { draftEditors, type DraftEditor } from '../../drafts/constants';
import { DraftRestoreDialog } from '../../drafts/DraftRestoreDialog';
import { formFieldsDraftSchema } from '../../drafts/schemas';
import { useEditorDraft } from '../../drafts/useEditorDraft';
import { forwardRef, useImperativeHandle, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { attributeValueTypes } from '../valueTypes';
import { EntityBlueprintSelect } from './EntityBlueprintSelect';
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
import { EntityFormAttributeEditor } from './EntityFormAttributeEditor';

export type EntityFormHandle = {
  /** Removes the persisted draft after a confirmed save. */
  clearDraft: () => void;
};

/** Identifies where unsaved field values are kept across refreshes. */
export type EntityFormDraft = {
  editor: DraftEditor;
  resource: readonly (string | number)[];
  /** Marker for the loaded source beyond the initial field values. */
  source: string;
};

const draftFieldSeparator = '\n';

const pickDraftFields = (
  fields: Record<string, string>,
  codes: readonly string[],
) =>
  Object.fromEntries(
    codes.flatMap((code) =>
      fields[code] === undefined ? [] : [[code, fields[code]]],
    ),
  );

// Creating and migrating an entity have no saved status edges, parent
// contexts or resolved values to show.
const noStatusParentContextIds: readonly string[] = [];
const noStatusTransitions: readonly [] = [];
const noResolvedValues = {};

type EntityFormProps = {
  expectedUpdatedAt?: string;
  blueprint?: BlueprintWithAttributes;
  draft?: EntityFormDraft;
  initialValues?: ReturnType<typeof valuesForForm>;
  contextId?: string | null;
  defaultContextId?: string | null;
  existingValues?: FormAttributeValue[];
  isLoadingBlueprint?: boolean;
  showAllAttributes?: boolean;
  highlightedAttributes?: readonly string[];
  migrationReviewMessages?: Readonly<Record<string, string>>;
  requiredAttributes?: readonly string[];
  /** Load or save failure; check violations are placed on their fields. */
  error?: Error | null;
  onLoadBlueprint?: (code: string, version?: number) => void;
  lockedBlueprint?: boolean;
  onSubmit: (input: {
    expected_updated_at?: string;
    values: ReturnType<typeof serializeAttributeValues>;
    relationships: ReturnType<typeof relationshipTargetsForForm>;
  }) => void;
  submitLabel: string;
};

/**
 * Enters a new entity's values, or the values of an entity being migrated to
 * another blueprint version. Existing entities are edited inline instead.
 */
export const EntityForm = forwardRef<EntityFormHandle, EntityFormProps>(
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
      showAllAttributes = false,
      highlightedAttributes = [],
      migrationReviewMessages = {},
      requiredAttributes = [],
      error,
      onLoadBlueprint,
      lockedBlueprint = false,
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
    // Entities are entered with the detail layout; each display component
    // edits with its paired edit component.
    const detailView = blueprint?.blueprint.views.detail;
    const fieldComponents = viewFieldEditComponents(detailView);
    const editableAttributes = editableFormAttributes(
      blueprint?.attributes ?? [],
      {
        contextId,
        defaultContextId,
        usesDefaultEditView: Boolean(
          blueprint && !showAllAttributes && !detailView,
        ),
      },
    );
    const fieldEditors = viewFieldEditors(fieldComponents, editableAttributes);
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
      blueprint && detailView && !showAllAttributes
        ? [
            ...new Set([
              ...unplacedEditableAttributes(editableAttributes, detailView),
              ...unplacedRequiredAttributes(editableAttributes, detailView, [
                ...requiredAttributes,
                ...(contextId === defaultContextId
                  ? entitySchemaRequiredAttributes(
                      blueprint.blueprint.entity_schema,
                    )
                  : []),
              ]),
            ]),
          ]
        : [];
    const validateFields = (fields: Record<string, string>) => {
      if (!blueprint) return { fieldErrors: {} };
      const validation = validateEntityForm(
        editableAttributes,
        fields,
        requiredAttributes,
        contextId === defaultContextId
          ? blueprint.blueprint.entity_schema
          : undefined,
        undefined,
        fieldEditors,
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
        const validation = validateFields(value.fields);
        if (
          Object.keys(validation.fieldErrors).length > 0 ||
          validation.formError
        )
          return;
        onSubmit({
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
    const draft = useEditorDraft({
      dirty: JSON.stringify(draftFields) !== initialDraftFields,
      // Unused without draft options; `ready` keeps the draft off.
      editor: draftOptions?.editor ?? draftEditors.entityCreate,
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

    const editorContext = {
      statusParentContextIds: noStatusParentContextIds,
      disabled: isLoadingBlueprint,
      contextId,
      defaultContextId,
      existingValues,
      statusSavedValues: savedValues,
      lockedAttributes,
      statusTransitions: noStatusTransitions,
      fieldErrors: { ...serverFieldErrors, ...fieldErrors },
      highlightedAttributes,
      migrationReviewMessages,
      resolvedValues: noResolvedValues,
    };

    return (
      <Paper
        component="form"
        noValidate
        onSubmit={(event) => {
          event.preventDefault();
          void form.handleSubmit();
        }}
        sx={{ mt: 4, p: 3 }}
      >
        <DraftRestoreDialog
          draft={draft.pending}
          onDiscard={draft.discard}
          onRestore={restoreDraft}
        />
        <Stack spacing={VIEW_EDIT_LAYOUT_SPACING}>
          {!blueprint && !lockedBlueprint && (
            <form.Field name="blueprintCode">
              {(field) => (
                <EntityBlueprintSelect
                  onChange={field.handleChange}
                  value={field.state.value}
                />
              )}
            </form.Field>
          )}
          {blueprint && (
            <Typography color="text.secondary">
              {t('entities.versionedCode', {
                code: blueprint.blueprint.code,
                version: blueprint.blueprint.version,
              })}
            </Typography>
          )}
          {blueprint && (
            <form.Field name="fields">
              {(field) => {
                const renderEditor = (
                  attribute: Attribute,
                  component?: ComponentReference | null,
                ) => (
                  <EntityFormAttributeEditor
                    {...editorContext}
                    attribute={attribute}
                    component={
                      resolveEditComponent(component) ??
                      fieldComponents.get(attribute.code)
                    }
                    required={requiredAttributes.includes(attribute.code)}
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
                );
                return (
                  <Box>
                    <EditableEntityLayout
                      attributes={blueprint.attributes}
                      fallbackVisibilityScope={
                        showAllAttributes
                          ? undefined
                          : detailView
                            ? 'detail'
                            : 'form'
                      }
                      headingAttributes={headingFields}
                      otherAttributes={otherAttributes}
                      renderEditor={renderEditor}
                      values={noResolvedValues}
                      view={showAllAttributes ? undefined : detailView}
                    />
                  </Box>
                );
              }}
            </form.Field>
          )}
          {error && (
            <ApiErrorAlert
              error={error}
              violations={serverViolations?.unplaced}
            />
          )}
          {formError && <Alert severity="error">{formError}</Alert>}
          <Button
            disabled={isLoadingBlueprint}
            type="submit"
            variant="contained"
          >
            {isLoadingBlueprint ? t('entities.loadingBlueprint') : submitLabel}
          </Button>
        </Stack>
      </Paper>
    );
  },
);
