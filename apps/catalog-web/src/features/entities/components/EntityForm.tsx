import { useForm, useStore } from '@tanstack/react-form';
import { Alert, Button, Paper, Stack, Typography } from '@mui/material';
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
  removedFormValues,
  smartFillFormFields,
  type RemovedAttributeValue,
  type ResolvedFormValues,
} from '../entityFormAttributes';
import { viewFieldComponents } from '../../views/viewFieldComponents';
import { resolveViewComponent } from '../../views/components/registry';
import { EntityView } from '../../views/components/EntityView';
import { draftEditors, type DraftEditor } from '../../drafts/constants';
import { DraftRestoreDialog } from '../../drafts/DraftRestoreDialog';
import { formFieldsDraftSchema } from '../../drafts/schemas';
import { useEditorDraft } from '../../drafts/useEditorDraft';
import {
  forwardRef,
  useImperativeHandle,
  useMemo,
  useState,
  type ReactNode,
} from 'react';
import { useTranslation } from 'react-i18next';
import { attributeValueTypes } from '../valueTypes';
import { EntityBlueprintSelect } from './EntityBlueprintSelect';
import {
  savedStatusValue,
  statusConfiguration,
  statusTransitionAllowed,
} from '../status';
import { EntityFormAttributeEditor } from './EntityFormAttributeEditor';

export type EntityFormHandle = {
  applySmartFillValues: (values: Record<string, string>) => void;
  /** Removes the persisted draft after a confirmed save. */
  clearDraft: () => void;
  getDraftValues: () => Record<string, string>;
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

type EntityFormProps = {
  expectedUpdatedAt?: string;
  statusParentContextIds?: readonly string[];
  blueprint?: BlueprintWithAttributes;
  draft?: EntityFormDraft;
  initialValues?: ReturnType<typeof valuesForForm>;
  contextId?: string | null;
  contextPicker?: ReactNode;
  defaultContextId?: string | null;
  footerActions?: ReactNode;
  existingValues?: FormAttributeValue[];
  reusableAttributes?: Attribute[];
  resolvedValues?: ResolvedFormValues;
  formId?: string;
  isLoadingBlueprint?: boolean;
  disabled?: boolean;
  showBlueprintMetadata?: boolean;
  showSubmitButton?: boolean;
  showAllAttributes?: boolean;
  highlightedAttributes?: readonly string[];
  migrationReviewMessages?: Readonly<Record<string, string>>;
  requiredAttributes?: readonly string[];
  entityId?: string;
  error?: Error | null;
  onLoadBlueprint?: (code: string, version?: number) => void;
  lockedBlueprint?: boolean;
  onSubmit: (input: {
    expected_updated_at?: string;
    values: ReturnType<typeof serializeAttributeValues>;
    relationships: ReturnType<typeof relationshipTargetsForForm>;
    remove_values: RemovedAttributeValue[];
  }) => void;
  submitLabel: string;
};

export const EntityForm = forwardRef<EntityFormHandle, EntityFormProps>(
  (
    {
      blueprint,
      expectedUpdatedAt,
      statusParentContextIds = [],
      draft: draftOptions,
      initialValues = {},
      contextId = null,
      contextPicker,
      defaultContextId = null,
      footerActions,
      existingValues = [],
      reusableAttributes = [],
      resolvedValues = {},
      formId,
      isLoadingBlueprint = false,
      disabled = false,
      showBlueprintMetadata = true,
      showSubmitButton = true,
      showAllAttributes = false,
      highlightedAttributes = [],
      migrationReviewMessages = {},
      requiredAttributes = [],
      entityId,
      error,
      onLoadBlueprint,
      lockedBlueprint = false,
      onSubmit,
      submitLabel,
    },
    ref,
  ) => {
    const { t } = useTranslation();
    const [savedVersion] = useState(expectedUpdatedAt);
    const [savedValues] = useState(existingValues);
    const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});
    const [formError, setFormError] = useState<string>();
    const editView = blueprint?.blueprint.views.edit;
    const fieldComponents = viewFieldComponents(editView);
    const fieldValidators = new Map(
      [...fieldComponents].flatMap(([code, component]) => {
        const definition = resolveViewComponent(component);
        const attribute = blueprint?.attributes.find(
          (item) => item.code === code,
        );
        return definition?.validateValue &&
          attribute &&
          definition.value_types.includes(attribute.value_type)
          ? [[code, definition.validateValue] as const]
          : [];
      }),
    );
    const editableAttributes = editableFormAttributes(
      blueprint ? [...blueprint.attributes, ...reusableAttributes] : [],
      {
        contextId,
        defaultContextId,
        usesDefaultEditView: Boolean(
          blueprint && !showAllAttributes && !editView,
        ),
      },
    );
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
        fieldValidators,
      );
      for (const attribute of editableAttributes) {
        const config = statusConfiguration(attribute);
        if (!config) continue;
        const parents =
          attribute.context_fallback === 'none' ? [] : statusParentContextIds;
        const before = savedStatusValue(attribute, savedValues, [
          contextId,
          ...parents,
        ]);
        const after =
          fields[attribute.code] ||
          savedStatusValue(attribute, savedValues, parents);
        if (!statusTransitionAllowed(config, before, after))
          validation.fieldErrors[attribute.code] = t(
            'entities.statusTransitionDenied',
          );
      }
      setFieldErrors(validation.fieldErrors);
      setFormError(validation.formError);
      return validation;
    };
    const form = useForm({
      defaultValues: {
        blueprintCode: blueprint?.blueprint.code ?? '',
        fields: initialValues,
      },
      onSubmit: ({ value }) => {
        if (disabled) return;
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
          ...(savedVersion ? { expected_updated_at: savedVersion } : {}),
          values: serializeAttributeValues(
            editableAttributes,
            value.fields,
            contextId,
          ),
          relationships: relationshipTargetsForForm(
            editableAttributes,
            value.fields,
            contextId,
          ),
          remove_values: removedFormValues(
            existingValues,
            editableAttributes,
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
      pickDraftFields(initialValues, draftCodes.split(draftFieldSeparator)),
    );
    const draft = useEditorDraft({
      dirty: JSON.stringify(draftFields) !== initialDraftFields,
      editor: draftOptions?.editor ?? draftEditors.entityEdit,
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

    useImperativeHandle(ref, () => ({
      clearDraft: draft.clear,
      getDraftValues: () => ({ ...form.state.values.fields }),
      applySmartFillValues: (values) => {
        const fields = smartFillFormFields(editableAttributes, values);
        if (Object.keys(fields).length === 0) return;
        const nextFields = { ...form.state.values.fields, ...fields };
        validateFields(nextFields);
        form.setFieldValue('fields', nextFields);
      },
    }));

    const editorContext = {
      statusParentContextIds,
      disabled: disabled || isLoadingBlueprint,
      contextId,
      defaultContextId,
      entityId,
      existingValues,
      statusSavedValues: savedValues,
      fieldErrors,
      highlightedAttributes,
      migrationReviewMessages,
      resolvedValues,
    };

    return (
      <Paper
        component="form"
        id={formId}
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
        <Stack spacing={2}>
          {contextPicker}
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
          {blueprint && showBlueprintMetadata && (
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
                    component={component ?? fieldComponents.get(attribute.code)}
                    required={requiredAttributes.includes(attribute.code)}
                    onChange={(nextValue) => {
                      const nextFields = {
                        ...field.state.value,
                        [attribute.code]: nextValue,
                      };
                      validateFields(nextFields);
                      field.handleChange(nextFields);
                    }}
                    value={field.state.value[attribute.code] ?? ''}
                  />
                );
                return (
                  <>
                    <EntityView
                      attributes={blueprint.attributes}
                      values={resolvedValues}
                      fallbackVisibilityScope={
                        showAllAttributes ? undefined : 'form'
                      }
                      view={showAllAttributes ? undefined : editView}
                      renderEditor={renderEditor}
                    />
                    {reusableAttributes.length > 0 && (
                      <>
                        <Typography sx={{ mt: 3 }} variant="h6">
                          {t('entities.additionalAttributes')}
                        </Typography>
                        <EntityView
                          attributes={reusableAttributes}
                          values={resolvedValues}
                          renderEditor={renderEditor}
                        />
                      </>
                    )}
                  </>
                );
              }}
            </form.Field>
          )}
          {(error || formError) && (
            <Alert severity="error">{error?.message ?? formError}</Alert>
          )}
          {footerActions}
          {showSubmitButton && (
            <Button
              disabled={disabled || isLoadingBlueprint}
              type="submit"
              variant="contained"
            >
              {isLoadingBlueprint
                ? t('entities.loadingBlueprint')
                : submitLabel}
            </Button>
          )}
        </Stack>
      </Paper>
    );
  },
);
