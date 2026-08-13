import { useForm } from '@tanstack/react-form';
import { useQuery } from '@tanstack/react-query';
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
  relationshipTargetsForForm,
  serializeAttributeValues,
  valuesForForm,
} from '../entity-form';
import { valueForField } from '../attribute-values';
import { displayLabel, dropdownOptionLabel } from '../entity-display';
import { entityQueryKeys } from '../query-keys';
import { attributeValueTypes } from '../value-types';

type EntityFormProps = {
  blueprint?: BlueprintWithAttributes;
  initialValues?: ReturnType<typeof valuesForForm>;
  contextId?: string | null;
  existingValues?: NewAttributeValue[];
  isLoadingBlueprint?: boolean;
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
  existingValues = [],
  isLoadingBlueprint = false,
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
            contextId === null || attribute.context_editable !== 'default',
        );
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
                  {blueprints.data?.map((option) => (
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
              <>
                {blueprint.attributes.map((attribute) => {
                  const value = field.state.value[attribute.code] ?? '';
                  const localValueExists = existingValues.some(
                    (item) =>
                      item.kind === 'scalar' &&
                      item.attribute_code === attribute.code &&
                      (item.context_id ?? null) === contextId,
                  );
                  const defaultValue = existingValues.find(
                    (item) =>
                      item.kind === 'scalar' &&
                      item.attribute_code === attribute.code &&
                      (item.context_id ?? null) === null,
                  );
                  const inherited =
                    contextId !== null &&
                    !localValueExists &&
                    attribute.context_fallback !== 'none' &&
                    defaultValue?.kind === 'scalar';
                  const defaultOnly =
                    contextId !== null &&
                    attribute.context_editable === 'default';
                  const helperText = defaultOnly
                    ? 'Managed in Default'
                    : inherited
                      ? `Using default: ${valueForField(defaultValue.value)}`
                      : undefined;
                  const handleChange = (nextValue: string) =>
                    field.handleChange({
                      ...field.state.value,
                      [attribute.code]: nextValue,
                    });
                  return attribute.value_type ===
                    attributeValueTypes.relationship ? (
                    <RelationshipField
                      key={attribute.code}
                      attribute={attribute}
                      disabled={defaultOnly}
                      onChange={handleChange}
                      value={value}
                    />
                  ) : attribute.value_type === attributeValueTypes.boolean ? (
                    <TextField
                      key={attribute.code}
                      fullWidth
                      disabled={defaultOnly}
                      helperText={helperText}
                      select
                      label={attribute.code}
                      onChange={(event) => handleChange(event.target.value)}
                      value={value}
                    >
                      <MenuItem value="">Not set</MenuItem>
                      <MenuItem value="true">True</MenuItem>
                      <MenuItem value="false">False</MenuItem>
                    </TextField>
                  ) : (
                    <TextField
                      key={attribute.code}
                      fullWidth
                      disabled={defaultOnly}
                      helperText={helperText}
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
                            attribute.value_type === attributeValueTypes.integer
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
                  );
                })}
              </>
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

const RelationshipField = ({
  attribute,
  disabled = false,
  onChange,
  value,
}: {
  attribute: Attribute;
  disabled?: boolean;
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
      <TextField
        fullWidth
        disabled={disabled}
        label={attribute.code}
        helperText="Comma-separated entity UUIDs"
        onChange={(event) => onChange(event.target.value)}
        value={value}
      />
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
    <FormControl fullWidth>
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
    </FormControl>
  );
};
