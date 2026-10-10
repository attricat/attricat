import { Box, Chip, List, Stack, TextField, Typography } from '@mui/material';
import { useInfiniteQuery } from '@tanstack/react-query';
import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { LoadMoreButton } from '../../components/LoadMoreButton';
import { searchRecords } from '../records/api';
import { useRelationshipSelectionLabels } from '../records/components/useRelationshipSelectionLabels';
import {
  scrollRelationshipPickerToTop,
  useRecentlyPreviewedRecords,
} from '../records/components/useRecentlyPreviewedRecords';
import { dropdownOptionLabel, displayLabel } from '../records/recordDisplay';
import { recordQueryKeys } from '../records/queryKeys';
import { RelationshipTargetOption } from './RelationshipTargetOption';

type Props = {
  onSelectedIdsChange: (ids: string[]) => void;
  selectedIds: string[];
  targetBlueprint: string;
};

export const RelationshipTargetPicker = ({
  onSelectedIdsChange,
  selectedIds,
  targetBlueprint,
}: Props) => {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const pickerContentRoot = useRef<HTMLDivElement>(null);
  const { isPreviewed, markPreviewed, openPreview, previewHref } =
    useRecentlyPreviewedRecords((id) => {
      if (!selectedIds.includes(id)) onSelectedIdsChange([...selectedIds, id]);
      scrollRelationshipPickerToTop(pickerContentRoot.current);
    });
  const targets = useInfiniteQuery({
    queryKey: recordQueryKeys.relationshipTargets(targetBlueprint, query),
    queryFn: ({ pageParam, signal }) =>
      searchRecords({
        blueprint: targetBlueprint,
        cursor: pageParam,
        query,
        signal,
      }),
    initialPageParam: null as string | null,
    getNextPageParam: (page) => page.next_cursor,
  });
  const options = targets.data?.pages.flatMap((page) => page.items) ?? [];
  const selectedLabels = useRelationshipSelectionLabels(
    targetBlueprint,
    selectedIds,
  );
  const views = targets.data?.pages[0]?.blueprint.blueprint.views ?? {};
  const toggle = (id: string) =>
    onSelectedIdsChange(
      selectedIds.includes(id)
        ? selectedIds.filter((selectedId) => selectedId !== id)
        : [...selectedIds, id],
    );

  return (
    <Stack ref={pickerContentRoot} spacing={1.5}>
      <TextField
        fullWidth
        label={t('records.searchRelationshipOptions')}
        onChange={(event) => setQuery(event.target.value)}
        value={query}
      />
      {selectedIds.length > 0 && (
        <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5 }}>
          {selectedIds.map((id) => (
            <Chip
              color="primary"
              key={id}
              label={selectedLabels.get(id) ?? id}
              onDelete={() => toggle(id)}
              size="small"
              variant="outlined"
            />
          ))}
        </Box>
      )}
      <List dense disablePadding>
        {options.map((target) => (
          <RelationshipTargetOption
            href={previewHref(target.id)}
            key={target.id}
            label={
              dropdownOptionLabel(target.preview, views) ??
              displayLabel(target.display, target.id)
            }
            onOpenPreview={() => openPreview(target.id)}
            onPreviewLinkClick={() => markPreviewed(target.id)}
            onToggle={() => toggle(target.id)}
            previewed={isPreviewed(target.id)}
            selected={selectedIds.includes(target.id)}
          />
        ))}
      </List>
      {targets.hasNextPage && (
        <LoadMoreButton
          isLoading={targets.isFetchingNextPage}
          onLoadMore={() => void targets.fetchNextPage()}
        />
      )}
      {targets.isPending && (
        <Typography variant="body2">{t('records.loadingOptions')}</Typography>
      )}
      {targets.isError && (
        <Typography color="error" variant="body2">
          {targets.error.message}
        </Typography>
      )}
    </Stack>
  );
};
