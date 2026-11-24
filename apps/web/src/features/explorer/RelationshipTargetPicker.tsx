import { Box, Button, Chip, MenuItem, Stack, TextField } from '@mui/material';
import { defaultStringifySearch } from '@tanstack/react-router';
import { useId, useRef, useState } from 'react';
import { IconLabel } from '../../components/IconLabel';
import { BlueprintIcon, ExplorerIcon } from '../../components/systemIcons';
import { relationshipPickerModes } from '../records/constants';
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
  // The Explorer, in a new tab, picks records of the shown blueprint with its
  // table selection and sends them back; see RelationshipPickerSelectionBar.
  const explorerHref = targetBlueprint
    ? `/${defaultStringifySearch({
        blueprint: targetBlueprint,
        locked: true,
        relationshipPicker: pickerToken,
        relationshipPickerMode: relationshipPickerModes.many,
      })}`
    : undefined;
  const toggle = (id: string) =>
    onSelectedIdsChange(
      selectedIds.includes(id)
        ? selectedIds.filter((selectedId) => selectedId !== id)
        : [...selectedIds, id],
    );

  return (
    <Stack ref={pickerContentRoot} spacing={1.5}>
      {targetBlueprints.length > 1 && (
        <TextField
          fullWidth
          label={t('records.relationshipTargetBlueprint')}
          onChange={(event) => setTargetBlueprint(event.target.value)}
          select
          value={targetBlueprint}
        >
          {targetBlueprints.map((code) => (
            <MenuItem key={code} value={code}>
              <IconLabel icon={BlueprintIcon}>{blueprintName(code)}</IconLabel>
            </MenuItem>
          ))}
        </TextField>
      )}
      {/* The Explorer is the other way to find targets, so it sits beside
          the search, as in the relationship field's picker. */}
      <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
        <SearchField
          fullWidth
          label={t('records.searchRelationshipOptions')}
          onChange={(event) => setQuery(event.target.value)}
          value={query}
        />
        {explorerHref && (
          <Button
            component="a"
            href={explorerHref}
            // The Explorer tab sends its selection back to this window.
            rel="opener"
            startIcon={<ExplorerIcon />}
            sx={{ alignSelf: 'stretch', flexShrink: 0, whiteSpace: 'nowrap' }}
            target="_blank"
            variant="outlined"
          >
            {t('records.chooseInExplorer')}
          </Button>
        )}
      </Stack>
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
