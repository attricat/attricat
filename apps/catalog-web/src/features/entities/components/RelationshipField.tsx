import { useState } from 'react';
import { useInfiniteQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  Button,
  Checkbox,
  FormControl,
  List,
  ListItemButton,
  ListItemText,
  Radio,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { LoadMoreButton } from '../../../components/LoadMoreButton';
import { RelationshipSelectorDialog } from '../../../components/RelationshipSelectorDialog';
import { RelationshipIcon } from '../../../components/system-icons';
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
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [draftIds, setDraftIds] = useState<string[]>([]);
  const [knownLabels, setKnownLabels] = useState<Record<string, string>>({});
  const targetBlueprint = attribute.target_blueprint_code;
  const selectedIds = value
    .split(',')
    .map((targetId) => targetId.trim())
    .filter(Boolean);
  const isOneToOne = attribute.relationship_cardinality === 'one_to_one';
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
    enabled: Boolean(targetBlueprint && open),
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

  const options = targets.data?.pages.flatMap((page) => page.items) ?? [];
  const targetViews = targets.data?.pages[0]?.blueprint.blueprint.views ?? {};
  const targetLabel = (target: (typeof options)[number]) =>
    dropdownOptionLabel(target.preview, targetViews) ??
    displayLabel(target.display, target.id);
  const labelById = new Map(
    options.map((target) => [target.id, targetLabel(target)]),
  );
  const labelForId = (id: string) => knownLabels[id] ?? labelById.get(id) ?? id;
  const availableOptions = options.filter(
    (target) => !draftIds.includes(target.id),
  );
  const openSelector = () => {
    setDraftIds(isOneToOne ? selectedIds.slice(0, 1) : selectedIds);
    setKnownLabels({});
    setQuery('');
    setOpen(true);
  };
  const selectTarget = (id: string, label: string) => {
    setKnownLabels((current) => ({ ...current, [id]: label }));
    setDraftIds((current) =>
      isOneToOne
        ? [id]
        : current.includes(id)
          ? current.filter((selectedId) => selectedId !== id)
          : [...current, id],
    );
  };

  return (
    <Stack spacing={0.5}>
      <FormControl error={Boolean(error)} fullWidth>
        <Button
          aria-label={attribute.code}
          disabled={disabled}
          onClick={openSelector}
          startIcon={<RelationshipIcon />}
          sx={{ justifyContent: 'space-between', minHeight: 56 }}
          variant="outlined"
        >
          <span>{attribute.code}</span>
          <Typography color="text.secondary" component="span" variant="body2">
            {t('entities.relationshipSelected', {
              count: selectedIds.length,
            })}
          </Typography>
        </Button>
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
      <RelationshipSelectorDialog
        applyLabel={t('entities.applyRelationshipSelection')}
        cancelLabel={t('entities.cancelRelationshipSelection')}
        clearLabel={t('entities.clearRelationshipSelection')}
        closeLabel={t('entities.closeRelationshipSelector')}
        onApply={() => {
          onChange(draftIds.join(', '));
          setOpen(false);
        }}
        onClear={() => setDraftIds([])}
        onClose={() => setOpen(false)}
        open={open}
        selectedLabel={t('entities.relationshipSelected', {
          count: draftIds.length,
        })}
        title={t(
          isOneToOne
            ? 'entities.selectOneRelationship'
            : 'entities.selectRelationships',
          { blueprint: targetBlueprint },
        )}
      >
        <Stack spacing={2}>
          <TextField
            fullWidth
            label={t('entities.searchRelationshipOptions')}
            onChange={(event) => setQuery(event.target.value)}
            value={query}
          />
          {draftIds.length > 0 && (
            <Stack spacing={0.5}>
              <Typography variant="subtitle2">
                {t('entities.selectedRelationships')}
              </Typography>
              <List dense disablePadding>
                {draftIds.map((id) => (
                  <ListItemButton
                    key={id}
                    onClick={() =>
                      setDraftIds((current) =>
                        current.filter((selectedId) => selectedId !== id),
                      )
                    }
                  >
                    {isOneToOne ? <Radio checked /> : <Checkbox checked />}
                    <ListItemText primary={labelForId(id)} />
                  </ListItemButton>
                ))}
              </List>
            </Stack>
          )}
          <Stack spacing={0.5}>
            <Typography variant="subtitle2">
              {t('entities.relationshipOptions')}
            </Typography>
            <List dense disablePadding>
              {availableOptions.map((target) => (
                <ListItemButton
                  key={target.id}
                  onClick={() => selectTarget(target.id, targetLabel(target))}
                >
                  {isOneToOne ? <Radio checked={false} /> : <Checkbox />}
                  <ListItemText primary={targetLabel(target)} />
                </ListItemButton>
              ))}
            </List>
            {!targets.isPending &&
              !targets.isError &&
              availableOptions.length === 0 && (
                <Typography color="text.secondary" variant="body2">
                  {t('entities.noRelationshipOptions')}
                </Typography>
              )}
            {targets.hasNextPage && (
              <LoadMoreButton
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
                {t('entities.couldNotLoadEntities', {
                  blueprint: targetBlueprint,
                })}
              </Typography>
            )}
          </Stack>
        </Stack>
      </RelationshipSelectorDialog>
    </Stack>
  );
};
