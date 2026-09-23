import { useForm } from '@tanstack/react-form';
import { useQuery } from '@tanstack/react-query';
import {
  Alert,
  Button,
  MenuItem,
  Paper,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import {
  listEntityBlueprints,
  type Attribute,
  type BlueprintWithAttributes,
  type FormAttributeValue,
} from '../api';
import {
  relationshipTargetsForForm,
  serializeAttributeValues,
  validateEntityForm,
  valuesForForm,
} from '../entity-form';
import { entityQueryKeys } from '../query-keys';
import { EntityView } from '../../views/components/EntityView';
import {
  forwardRef,
  useImperativeHandle,
  useState,
  type ReactNode,
} from 'react';
import { useTranslation } from 'react-i18next';
import { EntityAttributeEditor } from './EntityAttributeEditor';
import { isHiddenByDefault } from '../attribute-visibility';

export type EntityFormHandle = {
  applySmartFillValues: (values: Record<string, string>) => void;
};

type EntityFormProps = {
  blueprint?: BlueprintWithAttributes;
  initialValues?: ReturnType<typeof valuesForForm>;
  contextId?: string | null;
  contextPicker?: ReactNode;
  defaultContextId?: string | null;
  footerActions?: ReactNode;
  existingValues?: FormAttributeValue[];
  reusableAttributes?: Attribute[];
  resolvedValues?: Record<
    string,
    { value: unknown; source_context: { id: string; code: string } }
  >;
  formId?: string;
  isLoadingBlueprint?: boolean;
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
    values: ReturnType<typeof serializeAttributeValues>;
    relationships: ReturnType<typeof relationshipTargetsForForm>;
    remove_values: { attribute_code: string; context_id: string | null }[];
  }) => void;
  submitLabel: string;
};

export const EntityForm = forwardRef<EntityFormHandle, EntityFormProps>(
  (
    {
      blueprint,
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
    const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});
    const [formError, setFormError] = useState<string>();
    const editView = blueprint?.blueprint.views.edit;
    const usesDefaultEditView = Boolean(
      blueprint && !showAllAttributes && !editView,
    );
    const blueprints = useQuery({
      queryKey: entityQueryKeys.blueprints(),
      queryFn: ({ signal }) => listEntityBlueprints(signal),
      enabled: !blueprint,
    });
    const attributes = blueprint
      ? [...blueprint.attributes, ...reusableAttributes]
      : [];
    const editableAttributes = attributes.filter(
      (attribute) =>
        !attribute.readonly &&
        attribute.extension_type?.available !== false &&
        (contextId === defaultContextId ||
          attribute.context_editable !== 'default') &&
        (!usesDefaultEditView ||
          attribute.code.includes(':') ||
          !isHiddenByDefault(attribute, 'form')),
    );
    const validateFields = (fields: Record<string, string>) => {
      if (!blueprint) return { fieldErrors: {} };
      return validateEntityForm(
        editableAttributes,
        fields,
        requiredAttributes,
        contextId === defaultContextId
          ? blueprint.blueprint.entity_schema
          : undefined,
        {
          invalidRelationship: t('entities.invalidRelationshipValue'),
          invalidValue: t('entities.invalidAttributeValue'),
          required: t('entities.requiredAttributeValue'),
          schema: t('entities.schemaValidationFailed'),
        },
      );
    };
    const form = useForm({
      defaultValues: {
        blueprintCode: blueprint?.blueprint.code ?? '',
        fields: initialValues,
      },
      onSubmit: ({ value }) => {
        if (!blueprint && onLoadBlueprint) {
          onLoadBlueprint(value.blueprintCode);
          return;
        }
        if (blueprint) {
          const validation = validateFields(value.fields);
          setFieldErrors(validation.fieldErrors);
          setFormError(validation.formError);
          if (
            Object.keys(validation.fieldErrors).length > 0 ||
            validation.formError
          )
            return;
          onSubmit({
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
            remove_values: existingValues
              .filter(
                (item) =>
                  item.kind === 'scalar' &&
                  (item.context_id ?? null) === contextId &&
                  editableAttributes.some(
                    (attribute) => attribute.code === item.attribute_code,
                  ) &&
                  !value.fields[item.attribute_code]?.trim(),
              )
              .map((item) => ({
                attribute_code: item.attribute_code,
                context_id: contextId,
              })),
          });
        }
      },
    });

    useImperativeHandle(ref, () => ({
      applySmartFillValues: (values) => {
        const editableCodes = new Set(
          editableAttributes
            .filter(
              (attribute) =>
                attribute.value_type !== 'relationship' &&
                attribute.value_type !== 'file',
            )
            .map((attribute) => attribute.code),
        );
        const fields = Object.fromEntries(
          Object.entries(values).filter(([code]) => editableCodes.has(code)),
        );
        if (Object.keys(fields).length === 0) return;
        const nextFields = { ...form.state.values.fields, ...fields };
        const validation = validateFields(nextFields);
        setFieldErrors(validation.fieldErrors);
        setFormError(validation.formError);
        form.setFieldValue('fields', nextFields);
      },
    }));

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
        <Stack spacing={2}>
          {contextPicker}
          {!blueprint && !lockedBlueprint && (
            <>
              <Typography variant="h6">
                {t('entities.chooseBlueprint')}
              </Typography>
              <form.Field name="blueprintCode">
                {(field) => (
                  <TextField
                    select
                    required
                    label={t('entities.blueprint')}
                    onChange={(event) => field.handleChange(event.target.value)}
                    value={field.state.value}
                  >
                    <MenuItem value="">
                      {t('entities.selectBlueprint')}
                    </MenuItem>
                    {(blueprints.data ?? []).map((option) => (
                      <MenuItem key={option.code} value={option.code}>
                        {option.name} ({option.code})
                      </MenuItem>
                    ))}
                  </TextField>
                )}
              </form.Field>
              {blueprints.isError && (
                <Alert severity="error">
                  {t('entities.couldNotLoadBlueprints')}
                </Alert>
              )}
            </>
          )}
          {blueprint && showBlueprintMetadata && (
            <Typography color="text.secondary">
              {blueprint.blueprint.code} v{blueprint.blueprint.version}
            </Typography>
          )}
          {blueprint && (
            <form.Field name="fields">
              {(field) => {
                const renderEditor = (attribute: Attribute) => {
                  const value = field.state.value[attribute.code] ?? '';
                  const localValueExists = existingValues.some(
                    (item) =>
                      item.attribute_code === attribute.code &&
                      (item.context_id ?? null) === contextId,
                  );
                  const resolvedValue = resolvedValues[attribute.code];
                  const inherited =
                    !localValueExists &&
                    resolvedValue !== undefined &&
                    resolvedValue.source_context.id !== contextId;
                  const defaultOnly =
                    contextId !== defaultContextId &&
                    attribute.context_editable === 'default';
                  const readonly = attribute.readonly === true;
                  const requiresMigrationReview =
                    highlightedAttributes.includes(attribute.code);
                  const migrationReviewMessage =
                    migrationReviewMessages[attribute.code];
                  const helperText = readonly
                    ? t('entities.managedBySystem')
                    : defaultOnly
                      ? t('entities.managedInDefault')
                      : inherited
                        ? attribute.value_type === 'relationship'
                          ? t('entities.inheritedFromContext', {
                              context: resolvedValue.source_context.code,
                            })
                          : t('entities.inheritedValue', {
                              context: resolvedValue.source_context.code,
                              value:
                                typeof resolvedValue.value === 'object'
                                  ? JSON.stringify(resolvedValue.value)
                                  : String(resolvedValue.value),
                            })
                        : undefined;
                  const handleChange = (nextValue: string) => {
                    const nextFields = {
                      ...field.state.value,
                      [attribute.code]: nextValue,
                    };
                    const validation = validateFields(nextFields);
                    setFieldErrors(validation.fieldErrors);
                    setFormError(validation.formError);
                    field.handleChange(nextFields);
                  };
                  return (
                    <EntityAttributeEditor
                      attribute={attribute}
                      contextId={contextId}
                      disabled={readonly || defaultOnly}
                      entityId={entityId}
                      files={
                        existingValues.find(
                          (
                            item,
                          ): item is Extract<
                            FormAttributeValue,
                            { kind: 'file' }
                          > =>
                            item.kind === 'file' &&
                            item.attribute_code === attribute.code &&
                            (item.context_id ?? null) === contextId,
                        )?.files ?? []
                      }
                      error={fieldErrors[attribute.code]}
                      helperText={helperText}
                      migrationReviewMessage={migrationReviewMessage}
                      onChange={handleChange}
                      showMigrationBadge={requiresMigrationReview}
                      value={value}
                    />
                  );
                };
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
              disabled={isLoadingBlueprint}
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
