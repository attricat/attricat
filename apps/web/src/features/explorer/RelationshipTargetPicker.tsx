import { Box, Chip, Stack } from '@mui/material';
import { useId, useRef, useState } from 'react';
import { relationshipPickerModes } from '../records/constants';
import {
  RelationshipTargetSearchRow,
  TargetBlueprintSelect,
} from '../records/components/RelationshipTargetSearch';
import { relationshipPickerExplorerHref } from '../records/relationshipPickerHref';
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

type Props = {
  /** Names a target blueprint in the switcher between several. */
  blueprintName?: (code: string) => string;
  onSelectedIdsChange: (ids: string[]) => void;
  selectedIds: string[];
  /** The blueprints whose records can be chosen; a switcher picks among several. */
  targetBlueprints: readonly string[];
};

export const RelationshipTargetPicker = ({
  blueprintName = (code) => code,
  onSelectedIdsChange,
  selectedIds,
  targetBlueprints,
}: Props) => {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [targetBlueprint, setTargetBlueprint] = useState(targetBlueprints[0]);
  const pickerContentRoot = useRef<HTMLDivElement>(null);
  const optionName = useId();
  const { isPreviewed, markPreviewed, pickerToken, previewHref } =
    useRecentlyPreviewedRecords(
      (id) => {
        if (!selectedIds.includes(id))
          onSelectedIdsChange([...selectedIds, id]);
        scrollRelationshipPickerToTop(pickerContentRoot.current);
      },
      // Records chosen in an Explorer tab this picker opened join the filter.
      (ids) => {
        const added = ids.filter((id) => !selectedIds.includes(id));
        if (added.length > 0) onSelectedIdsChange([...selectedIds, ...added]);
        scrollRelationshipPickerToTop(pickerContentRoot.current);
      },
    );
  const { options, targetLabel, targets } = useRelationshipTargets(
    targetBlueprint,
    query,
    true,
  );
  const selectedLabels = useRelationshipSelectionLabels(selectedIds);
  const explorerHref = targetBlueprint
    ? relationshipPickerExplorerHref({
        blueprint: targetBlueprint,
        mode: relationshipPickerModes.filter,
        token: pickerToken,
      })
    : undefined;
  const toggle = (id: string) =>
    onSelectedIdsChange(
      selectedIds.includes(id)
        ? selectedIds.filter((selectedId) => selectedId !== id)
        : [...selectedIds, id],
    );

  return (
    <Stack ref={pickerContentRoot} spacing={1.5}>
      <TargetBlueprintSelect
        blueprintName={blueprintName}
        onChange={setTargetBlueprint}
        targets={targetBlueprints}
        value={targetBlueprint}
      />
      <RelationshipTargetSearchRow
        explorerHref={explorerHref}
        onQueryChange={setQuery}
        query={query}
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
