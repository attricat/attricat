import { Box, Chip, Stack } from '@mui/material';
import { useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { EmptyState } from '../../components/EmptyState';
import { NextPageButton } from '../../components/LoadMoreButton';
import { RecordIcon } from '../../components/systemIcons';
import { useRelationshipSelectionLabels } from '../records/components/useRelationshipSelectionLabels';
import {
  scrollRelationshipPickerToTop,
  useRecentlyPreviewedRecords,
} from '../records/components/useRecentlyPreviewedRecords';
import {
  RelationshipTargetList,
  RelationshipTargetOption,
} from '../records/components/RelationshipTargetOption';
import { useRelationshipTargets } from '../records/components/useRelationshipTargets';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { LoadingState } from '../../components/LoadingState';
import { SearchField } from '../../components/SearchField';

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
  const optionName = useId();
  const { isPreviewed, markPreviewed, previewHref } =
    useRecentlyPreviewedRecords((id) => {
      if (!selectedIds.includes(id)) onSelectedIdsChange([...selectedIds, id]);
      scrollRelationshipPickerToTop(pickerContentRoot.current);
    });
  const { options, targetLabel, targets } = useRelationshipTargets(
    targetBlueprint,
    query,
    true,
  );
  const selectedLabels = useRelationshipSelectionLabels(selectedIds);
  const toggle = (id: string) =>
    onSelectedIdsChange(
      selectedIds.includes(id)
        ? selectedIds.filter((selectedId) => selectedId !== id)
        : [...selectedIds, id],
    );

  return (
    <Stack ref={pickerContentRoot} spacing={1.5}>
      <SearchField
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
      <RelationshipTargetList
        isSingle={false}
        label={t('records.relationshipOptions')}
      >
        {options.map((target) => (
          <RelationshipTargetOption
            isSample={target.is_sample === true}
            isSingle={false}
            key={target.id}
            label={targetLabel(target)}
            name={optionName}
            onPreview={() => markPreviewed(target.id)}
            onSelect={() => toggle(target.id)}
            previewHref={previewHref(target.id)}
            previewed={isPreviewed(target.id)}
            selected={selectedIds.includes(target.id)}
          />
        ))}
      </RelationshipTargetList>
      {!targets.isPending && !targets.isError && options.length === 0 && (
        <EmptyState
          icon={RecordIcon}
          title={t('records.noRelationshipOptions')}
        />
      )}
      <NextPageButton query={targets} />
      {targets.isPending && (
        <LoadingState>{t('records.loadingOptions')}</LoadingState>
      )}
      <ApiErrorAlert error={targets.error} />
    </Stack>
  );
};
