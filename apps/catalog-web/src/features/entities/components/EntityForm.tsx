import { useForm } from '@tanstack/react-form';
import { useQuery } from '@tanstack/react-query';
import InfoOutlinedIcon from '@mui/icons-material/InfoOutlined';
import {
  Alert,
  Button,
  Checkbox,
  FormControl,
  InputLabel,
  ListItemText,
  MenuItem,
  Paper,
  Select,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import {
  listEntityBlueprints,
  searchEntities,
  type Attribute,
  type BlueprintWithAttributes,
  type NewAttributeValue,
} from '../api';
import {
  hasInvalidScalarField,
  relationshipTargetsForForm,
  serializeAttributeValues,
  valuesForForm,
} from '../entity-form';
import { displayLabel, dropdownOptionLabel } from '../entity-display';
import { entityQueryKeys } from '../query-keys';
import { attributeValueTypes } from '../value-types';
import { scalarValueForField } from '../attribute-values';
import { EntityView } from '../../views/components/EntityView';

type EntityFormProps = {
  blueprint?: BlueprintWithAttributes;
  initialValues?: ReturnType<typeof valuesForForm>;
  contextId?: string | null;
  defaultContextId?: string | null;
  existingValues?: NewAttributeValue[];
  resolvedValues?: Record<
    string,
    { value: unknown; source_context: { id: string; code: string } }
  >;
  isLoadingBlueprint?: boolean;
  showAllAttributes?: boolean;
  highlightedAttributes?: readonly string[];
  migrationReviewMessages?: Readonly<Record<string, string>>;
  requiredAttributes?: readonly string[];
  error?: Error | null;
  onLoadBlueprint?: (code: string, version?: number) => void;
  onSubmit: (input: {
    values: ReturnType<typeof serializeAttributeValues>;
    relationships: ReturnType<typeof relationshipTargetsForForm>;
    remove_values: { attribute_code: string; context_id: string | null }[];
  }) => void;
  submitLabel: string;
};

export const EntityForm = ({
  blueprint,
  initialValues = {},
  contextId = null,
  defaultContextId = null,
  existingValues = [],
  resolvedValues = {},
  isLoadingBlueprint = false,
  showAllAttributes = false,
  highlightedAttributes = [],
  migrationReviewMessages = {},
  requiredAttributes = [],
  error,
  onLoadBlueprint,
  onSubmit,
  submitLabel,
}: EntityFormProps) => {
  const blueprints = useQuery({
    queryKey: entityQueryKeys.blueprints(),
    queryFn: listEntityBlueprints,
    enabled: !blueprint,
  });
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
        const editableAttributes = blueprint.attributes.filter(
          (attribute) =>
            contextId === defaultContextId ||
            attribute.context_editable !== 'default',
        );
        if (
          requiredAttributes.some(
            (attributeCode) => !value.fields[attributeCode]?.trim(),
          )
        )
          return;
        if (hasInvalidScalarField(editableAttributes, value.fields)) return;
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

  return (
    <Paper
      component="form"
      onSubmit={(event) => {
        event.preventDefault();
        void form.handleSubmit();
      }}
      sx={{ mt: 4, p: 3 }}
    >
      <Stack spacing={2}>
        {!blueprint && (
          <>
            <Typography variant="h6">Choose blueprint</Typography>
            <form.Field name="blueprintCode">
              {(field) => (
                <TextField
                  select
                  required
                  label="Blueprint"
                  onChange={(event) => field.handleChange(event.target.value)}
                  value={field.state.value}
                >
                  <MenuItem value="">Select a blueprint</MenuItem>
                  {(blueprints.data ?? []).map((option) => (
                    <MenuItem key={option.code} value={option.code}>
                      {option.name} ({option.code})
                    </MenuItem>
                  ))}
                </TextField>
              )}
            </form.Field>
            {blueprints.isError && (
              <Alert severity="error">Could not load blueprints.</Alert>
            )}
          </>
        )}
        {blueprint && (
          <Typography color="text.secondary">
            {blueprint.blueprint.code} v{blueprint.blueprint.version}
          </Typography>
        )}
        {blueprint && (
          <form.Field name="fields">
            {(field) => (
              <EntityView
                attributes={blueprint.attributes}
                values={resolvedValues}
                view={
                  showAllAttributes ? undefined : blueprint.blueprint.views.edit
                }
                renderEditor={(attribute) => {
                  const value = field.state.value[attribute.code] ?? '';
                  const localValueExists = existingValues.some(
                    (item) =>
                      item.kind === 'scalar' &&
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
                  const invalid =
                    attribute.value_type !== attributeValueTypes.relationship &&
                    Boolean(value.trim()) &&
                    !scalarValueForField(attribute, value);
                  const missingRequired =
                    requiredAttributes.includes(attribute.code) &&
                    !value.trim();
                  const requiresMigrationReview =
                    highlightedAttributes.includes(attribute.code);
                  const migrationReviewMessage =
                    migrationReviewMessages[attribute.code];
                  const helperText = defaultOnly
                    ? 'Managed in Default'
                    : inherited
                      ? `Using ${resolvedValue.source_context.code}: ${typeof resolvedValue.value === 'object' ? JSON.stringify(resolvedValue.value) : String(resolvedValue.value)}`
                      : undefined;
                  const fieldHelperText = invalid
                    ? "Enter a value that meets this field's requirements."
                    : missingRequired
                      ? 'A value is required for the target schema.'
                      : helperText;
                  const handleChange = (nextValue: string) =>
                    field.handleChange({
                      ...field.state.value,
                      [attribute.code]: nextValue,
                    });
                  return attribute.value_type ===
                    attributeValueTypes.relationship ? (
                    <RelationshipField
                      attribute={attribute}
                      disabled={defaultOnly}
                      showMigrationBadge={requiresMigrationReview}
                      migrationReviewMessage={migrationReviewMessage}
                      error={missingRequired}
                      onChange={handleChange}
                      value={value}
                    />
                  ) : attribute.value_type === attributeValueTypes.boolean ? (
                    <>
                      {requiresMigrationReview && (
                        <MigrationBadge message={migrationReviewMessage} />
                      )}
                      <TextField
                        fullWidth
                        disabled={defaultOnly}
                        error={missingRequired}
                        helperText={fieldHelperText}
                        select
                        label={attribute.code}
                        onChange={(event) => handleChange(event.target.value)}
                        value={value}
                      >
                        <MenuItem value="">Not set</MenuItem>
                        <MenuItem value="true">True</MenuItem>
                        <MenuItem value="false">False</MenuItem>
                      </TextField>
                    </>
                  ) : (
                    <>
                      {requiresMigrationReview && (
                        <MigrationBadge message={migrationReviewMessage} />
                      )}
                      <TextField
                        fullWidth
                        disabled={defaultOnly}
                        error={invalid || missingRequired}
                        helperText={fieldHelperText}
                        label={attribute.code}
                        onChange={(event) => handleChange(event.target.value)}
                        placeholder={
                          attribute.value_type === attributeValueTypes.time
                            ? '09:30:00 America/New_York'
                            : undefined
                        }
                        slotProps={{
                          htmlInput: {
                            inputMode:
                              attribute.value_type ===
                                attributeValueTypes.number ||
                              attribute.value_type ===
                                attributeValueTypes.integer
                                ? 'decimal'
                                : undefined,
                          },
                        }}
                        type={
                          attribute.value_type === attributeValueTypes.date
                            ? 'date'
                            : attribute.value_type ===
                                  attributeValueTypes.number ||
                                attribute.value_type ===
                                  attributeValueTypes.integer
                              ? 'number'
                              : undefined
                        }
                        value={value}
                      />
                    </>
                  );
                }}
              />
            )}
          </form.Field>
        )}
        {error && <Alert severity="error">{error.message}</Alert>}
        <Button disabled={isLoadingBlueprint} type="submit" variant="contained">
          {isLoadingBlueprint ? 'Loading blueprint...' : submitLabel}
        </Button>
      </Stack>
    </Paper>
  );
};

const MigrationBadge = ({ message }: { message?: string }) => (
  <Tooltip
    title={
      message ??
      'Review is necessary for this field to migrate to the current schema version.'
    }
  >
    <InfoOutlinedIcon color="info" fontSize="small" />
  </Tooltip>
);

const RelationshipField = ({
  attribute,
  disabled = false,
  showMigrationBadge = false,
  migrationReviewMessage,
  error = false,
  onChange,
  value,
}: {
  attribute: Attribute;
  disabled?: boolean;
  showMigrationBadge?: boolean;
  migrationReviewMessage?: string;
  error?: boolean;
  onChange: (value: string) => void;
  value: string;
}) => {
  const targetBlueprint = attribute.target_blueprint_code;
  const targets = useQuery({
    queryKey: entityQueryKeys.relationshipTargets(targetBlueprint),
    queryFn: () => searchEntities(targetBlueprint!, undefined, ''),
    enabled: Boolean(targetBlueprint),
  });
  if (!targetBlueprint) {
    return (
      <Stack spacing={0.5}>
        {showMigrationBadge && (
          <MigrationBadge message={migrationReviewMessage} />
        )}
        <TextField
          fullWidth
          disabled={disabled}
          error={error}
          label={attribute.code}
          helperText={
            error
              ? 'A value is required for the target schema.'
              : 'Comma-separated entity UUIDs'
          }
          onChange={(event) => onChange(event.target.value)}
          value={value}
        />
      </Stack>
    );
  }

  const selectedIds = value
    .split(',')
    .map((targetId) => targetId.trim())
    .filter(Boolean);
  const options = [...(targets.data?.items ?? [])];
  for (const targetId of selectedIds) {
    if (!options.some((target) => target.id === targetId)) {
      options.push({
        id: targetId,
        blueprint_version: 0,
        schema_outdated: false,
        display: { default: targetId },
        preview: {},
      });
    }
  }
  const targetDisplay = targets.data?.blueprint.blueprint.display ?? {};
  const targetLabel = (target: (typeof options)[number]) =>
    dropdownOptionLabel(target.preview, targetDisplay) ??
    displayLabel(target.display, target.id);
  const labels = new Map(
    options.map((target) => [target.id, targetLabel(target)]),
  );

  return (
    <Stack spacing={0.5}>
      {showMigrationBadge && (
        <MigrationBadge message={migrationReviewMessage} />
      )}
      <FormControl error={error} fullWidth>
        <InputLabel id={`${attribute.code}-label`}>{attribute.code}</InputLabel>
        <Select
          disabled={disabled}
          multiple
          label={attribute.code}
          labelId={`${attribute.code}-label`}
          onChange={(event) =>
            onChange((event.target.value as string[]).join(', '))
          }
          renderValue={(selected) =>
            (selected as string[])
              .map((targetId) => labels.get(targetId) ?? targetId)
              .join(', ')
          }
          value={selectedIds}
        >
          {options.map((target) => (
            <MenuItem key={target.id} value={target.id}>
              <Checkbox checked={selectedIds.includes(target.id)} />
              <ListItemText primary={targetLabel(target)} />
            </MenuItem>
          ))}
        </Select>
        {targets.isPending && (
          <Typography variant="caption">Loading options...</Typography>
        )}
        {targets.isError && (
          <Typography color="error" variant="caption">
            Could not load {targetBlueprint} entities.
          </Typography>
        )}
        {error && (
          <Typography color="error" variant="caption">
            A value is required for the target schema.
          </Typography>
        )}
      </FormControl>
    </Stack>
  );
};
