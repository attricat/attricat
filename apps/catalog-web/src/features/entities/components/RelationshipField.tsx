import { useQuery } from '@tanstack/react-query';
import {
  Checkbox,
  FormControl,
  InputLabel,
  ListItemText,
  MenuItem,
  Select,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { searchEntities, type Attribute } from '../api';
import { displayLabel, dropdownOptionLabel } from '../entity-display';
import { entityQueryKeys } from '../query-keys';

export const RelationshipField = ({
  attribute,
  disabled = false,
  error,
  onChange,
  value,
}: {
  attribute: Attribute;
  disabled?: boolean;
  error?: string;
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
        error={Boolean(error)}
        label={attribute.code}
        helperText={error ?? 'Comma-separated entity UUIDs'}
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
      <FormControl error={Boolean(error)} fullWidth>
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
            {error}
          </Typography>
        )}
      </FormControl>
    </Stack>
  );
};
