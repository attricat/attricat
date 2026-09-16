import { useRef, useState } from 'react';
import { useInfiniteQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  Box,
  Button,
  Chip,
  FormControl,
  Link,
  List,
  ListItem,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { compactOutlinedActionButtonSx } from '../../../components/CompactOutlinedActionButton';
import { LoadMoreButton } from '../../../components/LoadMoreButton';
import { RelationshipSelectorDialog } from '../../../components/RelationshipSelectorDialog';
import { RelationshipPickerIcon } from '../../../components/system-icons';
import { searchEntities, type Attribute } from '../api';
import { RelationshipSelectionPills } from './RelationshipSelectionPills';
import { useRelationshipSelectionLabels } from './useRelationshipSelectionLabels';
import {
  scrollRelationshipPickerToTop,
  useRecentlyPreviewedEntities,
} from './useRecentlyPreviewedEntities';
import {
  attributeLabel,
  displayLabel,
  dropdownOptionLabel,
} from '../entity-display';
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
  const pickerContentRoot = useRef<HTMLDivElement>(null);
  const targetBlueprint = attribute.target_blueprint_code;
  const selectedIds = value
    .split(',')
    .map((targetId) => targetId.trim())
    .filter(Boolean);
  const isSingle = attribute.cardinality === 'one';
  const selectionLabels = useRelationshipSelectionLabels(
    targetBlueprint,
    selectedIds,
  );
  const { isPreviewed, markPreviewed, openPreview, previewHref } =
    useRecentlyPreviewedEntities((id) => {
      setDraftIds((current) =>
        isSingle ? [id] : current.includes(id) ? current : [...current, id],
      );
      scrollRelationshipPickerToTop(pickerContentRoot.current);
    });
  const targets = useInfiniteQuery({
    queryKey: entityQueryKeys.relationshipTargets(targetBlueprint, query),
    queryFn: ({ pageParam, signal }) =>
      searchEntities({
        blueprint: targetBlueprint!,
        cursor: pageParam,
        query,
        signal,
      }),
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
        label={attributeLabel(attribute)}
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
  const labelForId = (id: string) =>
    knownLabels[id] ?? labelById.get(id) ?? selectionLabels.get(id) ?? id;
  const availableOptions = options.filter(
    (target) => !draftIds.includes(target.id),
  );
  const openSelector = () => {
    setDraftIds(isSingle ? selectedIds.slice(0, 1) : selectedIds);
    setKnownLabels({});
    setQuery('');
    setOpen(true);
  };
  const selectTarget = (id: string, label: string) => {
    setKnownLabels((current) => ({ ...current, [id]: label }));
    setDraftIds((current) =>
      isSingle
        ? [id]
        : current.includes(id)
          ? current.filter((selectedId) => selectedId !== id)
          : [...current, id],
    );
  };

  return (
    <Stack spacing={0.5}>
      <FormControl error={Boolean(error)} fullWidth>
        <Stack spacing={0.5}>
          <Typography variant="body2">{attributeLabel(attribute)}</Typography>
          <RelationshipSelectionPills
            action={
              <Button
                aria-label={attribute.code}
                color="primary"
                disabled={disabled}
                onClick={openSelector}
                size="small"
                startIcon={<RelationshipPickerIcon fontSize="small" />}
                sx={compactOutlinedActionButtonSx}
                variant="outlined"
              >
                {t('entities.openRelationshipSelector')}
              </Button>
            }
            ids={selectedIds}
            labels={selectionLabels}
            onRemove={
              disabled
                ? undefined
                : (id) =>
                    onChange(
                      selectedIds
                        .filter((selectedId) => selectedId !== id)
                        .join(', '),
                    )
            }
          />
        </Stack>
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
        actions={{
          applyLabel: t('entities.applyRelationshipSelection'),
          cancelLabel: t('entities.cancelRelationshipSelection'),
          clearLabel: t('entities.clearRelationshipSelection'),
          onApply: () => {
            onChange(draftIds.join(', '));
            setOpen(false);
          },
          onClear: () => setDraftIds([]),
        }}
        closeLabel={t('entities.closeRelationshipSelector')}
        onClose={() => setOpen(false)}
        open={open}
        selectedLabel={t('entities.relationshipSelected', {
          count: draftIds.length,
        })}
        title={t(
          isSingle
            ? 'entities.selectOneRelationship'
            : 'entities.selectRelationships',
          { blueprint: targetBlueprint },
        )}
      >
        <Stack ref={pickerContentRoot} spacing={2}>
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
              <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5 }}>
                {draftIds.map((id) => (
                  <Chip
                    color="primary"
                    key={id}
                    label={labelForId(id)}
                    onDelete={() =>
                      setDraftIds((current) =>
                        current.filter((selectedId) => selectedId !== id),
                      )
                    }
                    size="small"
                    variant="outlined"
                  />
                ))}
              </Box>
            </Stack>
          )}
          <Stack spacing={0.5}>
            <Typography variant="subtitle2">
              {t('entities.relationshipOptions')}
            </Typography>
            <List dense disablePadding>
              {availableOptions.map((target) => (
                <ListItem
                  key={target.id}
                  sx={{
                    alignItems: 'center',
                    bgcolor: isPreviewed(target.id)
                      ? 'action.selected'
                      : undefined,
                    borderRadius: 1,
                    '&:hover': { bgcolor: 'action.hover' },
                  }}
                >
                  <Link
                    href={previewHref(target.id)}
                    onClick={() => markPreviewed(target.id)}
                    rel="opener"
                    sx={{ flexGrow: 1, minWidth: 0, mr: 1 }}
                    target="_blank"
                  >
                    {targetLabel(target)}
                  </Link>
                  <Stack direction="row" spacing={0.5}>
                    <Button
                      aria-label={t('entities.previewRelationshipOptionLabel', {
                        option: targetLabel(target),
                      })}
                      color={isPreviewed(target.id) ? 'secondary' : 'inherit'}
                      onClick={() => openPreview(target.id)}
                      size="small"
                    >
                      {t('entities.previewRelationshipOption')}
                    </Button>
                    <Button
                      aria-label={t('entities.selectRelationshipOptionLabel', {
                        option: targetLabel(target),
                      })}
                      onClick={() =>
                        selectTarget(target.id, targetLabel(target))
                      }
                      size="small"
                    >
                      {t('entities.selectRelationshipOption')}
                    </Button>
                  </Stack>
                </ListItem>
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
