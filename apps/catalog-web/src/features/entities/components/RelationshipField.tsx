import { useState } from 'react';
import { useInfiniteQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  Autocomplete,
  Checkbox,
  FormControl,
  ListItemText,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { LoadMoreButton } from '../../../components/LoadMoreButton';
import { searchEntities, type Attribute } from '../api';
import { displayLabel, dropdownOptionLabel } from '../entity-display';
import { entityQueryKeys } from '../query-keys';

export const RelationshipField = ({
  attribute,
  disabled = false,
  error,
  helperText,
  onChange,
  value,
}: {
  attribute: Attribute;
  disabled?: boolean;
  error?: string;
  helperText?: string;
  onChange: (value: string) => void;
  value: string;
}) => {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const targetBlueprint = attribute.target_blueprint_code;
  const targets = useInfiniteQuery({
    queryKey: entityQueryKeys.relationshipTargets(targetBlueprint, query),
    queryFn: ({ pageParam, signal }) =>
      searchEntities(
        targetBlueprint!,
        undefined,
        query,
        pageParam,
        undefined,
        signal,
      ),
    initialPageParam: null as string | null,
    getNextPageParam: (page) => page.next_cursor,
    enabled: Boolean(targetBlueprint),
  });
  if (!targetBlueprint) {
    return (
      <TextField
        fullWidth
        disabled={disabled}
        error={Boolean(error)}
        label={attribute.code}
        helperText={error ?? t('entities.commaSeparatedUuids')}
        onChange={(event) => onChange(event.target.value)}
        value={value}
      />
    );
  }

  const selectedIds = value
    .split(',')
    .map((targetId) => targetId.trim())
    .filter(Boolean);
  const options = targets.data?.pages.flatMap((page) => page.items) ?? [];
  for (const targetId of selectedIds) {
    if (!options.some((target) => target.id === targetId)) {
      options.push({
        id: targetId,
        blueprint_version: 0,
        schema_outdated: false,
        display: { default: targetId },
        match_explanations: [],
        preview: {},
      });
    }
  }
  const targetViews = targets.data?.pages[0]?.blueprint.blueprint.views ?? {};
  const targetLabel = (target: (typeof options)[number]) =>
    dropdownOptionLabel(target.preview, targetViews) ??
    displayLabel(target.display, target.id);
  const selectedTargets = selectedIds.flatMap((targetId) => {
    const target = options.find((option) => option.id === targetId);
    return target ? [target] : [];
  });

  const isOneToOne = attribute.relationship_cardinality === 'one_to_one';

  return (
    <Stack spacing={0.5}>
      <FormControl error={Boolean(error)} fullWidth>
        {isOneToOne ? (
          <Autocomplete
            disabled={disabled}
            filterOptions={(items) => items}
            getOptionLabel={targetLabel}
            inputValue={query}
            isOptionEqualToValue={(option, selected) =>
              option.id === selected.id
            }
            onChange={(_, selected) => onChange(selected?.id ?? '')}
            onInputChange={(_, input, reason) => {
              if (reason === 'input' || reason === 'clear') setQuery(input);
            }}
            options={options}
            renderInput={(params) => (
              <TextField
                {...params}
                error={Boolean(error)}
                label={attribute.code}
              />
            )}
            renderOption={(props, target) => (
              <li {...props} key={target.id}>
                <ListItemText primary={targetLabel(target)} />
              </li>
            )}
            value={selectedTargets[0] ?? null}
          />
        ) : (
          <Autocomplete
            disableCloseOnSelect
            disabled={disabled}
            filterOptions={(items) => items}
            getOptionLabel={targetLabel}
            inputValue={query}
            isOptionEqualToValue={(option, selected) =>
              option.id === selected.id
            }
            multiple
            onChange={(_, selected) =>
              onChange(selected.map((target) => target.id).join(', '))
            }
            onInputChange={(_, input, reason) => {
              if (reason === 'input' || reason === 'clear') setQuery(input);
            }}
            options={options}
            renderInput={(params) => (
              <TextField
                {...params}
                error={Boolean(error)}
                label={attribute.code}
              />
            )}
            renderOption={(props, target, { selected }) => (
              <li {...props} key={target.id}>
                <Checkbox checked={selected} />
                <ListItemText primary={targetLabel(target)} />
              </li>
            )}
            value={selectedTargets}
          />
        )}
        {targets.hasNextPage && (
          <LoadMoreButton
            disabled={disabled}
            isLoading={targets.isFetchingNextPage}
            onLoadMore={() => void targets.fetchNextPage()}
          />
        )}
        {targets.isPending && (
          <Typography variant="caption">
            {t('entities.loadingOptions')}
          </Typography>
        )}
        {targets.isError && (
          <Typography color="error" variant="caption">
            {t('entities.couldNotLoadEntities', { blueprint: targetBlueprint })}
          </Typography>
        )}
        {error && (
          <Typography color="error" variant="caption">
            {error}
          </Typography>
        )}
        {helperText && (
          <Typography color="text.secondary" variant="caption">
            {helperText}
          </Typography>
        )}
      </FormControl>
    </Stack>
  );
};
