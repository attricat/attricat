import { Box, Chip, List, Stack, TextField, Typography } from '@mui/material';
import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { NextPageButton } from '../../components/LoadMoreButton';
import { useRelationshipSelectionLabels } from '../records/components/useRelationshipSelectionLabels';
import {
  scrollRelationshipPickerToTop,
  useRecentlyPreviewedRecords,
} from '../records/components/useRecentlyPreviewedRecords';
import { RelationshipTargetOption } from '../records/components/RelationshipTargetOption';
import { useRelationshipTargets } from '../records/components/useRelationshipTargets';
import { errorMessage } from '../../api/errors';

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
  const { options, targetLabel, targets } = useRelationshipTargets(
    targetBlueprint,
    query,
    true,
  );
  const selectedLabels = useRelationshipSelectionLabels(
    targetBlueprint,
    selectedIds,
  );
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
            isSample={target.is_sample === true}
            key={target.id}
            label={targetLabel(target)}
            onPreview={() => openPreview(target.id)}
            onPreviewLinkClick={() => markPreviewed(target.id)}
            onSelect={() => toggle(target.id)}
            previewHref={previewHref(target.id)}
            previewed={isPreviewed(target.id)}
            selected={selectedIds.includes(target.id)}
          />
        ))}
      </List>
      <NextPageButton query={targets} />
      {targets.isPending && (
        <Typography variant="body2">{t('records.loadingOptions')}</Typography>
      )}
      {targets.isError && (
        <Typography color="error" variant="body2">
          {errorMessage(targets.error)}
        </Typography>
      )}
    </Stack>
  );
};
