import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { List, MenuItem, Stack, TextField, Typography } from '@mui/material';
import { LoadMoreButton } from '../../../components/LoadMoreButton';
import { RelationshipSelectorDialog } from '../../../components/RelationshipSelectorDialog';
import { RelationshipDraftSelection } from './RelationshipDraftSelection';
import { RelationshipTargetOption } from './RelationshipTargetOption';
import { useRelationshipTargets } from './useRelationshipTargets';
import {
  scrollRelationshipPickerToTop,
  useRecentlyPreviewedRecords,
} from './useRecentlyPreviewedRecords';

/**
 * Picks the records a relationship field links to. Mount it with a new `key`
 * for each opening: the draft selection, search and target start from
 * `initialIds` and are only handed back through `onApply`.
 */
export const RelationshipPickerDialog = ({
  allowedTargets,
  initialIds,
  isSingle,
  onApply,
  onClose,
  open,
  selectionLabels,
}: {
  /** Blueprints the relationship may link to; the first is searched first. */
  allowedTargets: readonly string[];
  initialIds: readonly string[];
  isSingle: boolean;
  onApply: (ids: string[]) => void;
  onClose: () => void;
  open: boolean;
  /** Labels of the records already selected in the field. */
  selectionLabels: ReadonlyMap<string, string>;
}) => {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [draftIds, setDraftIds] = useState<string[]>(() =>
    isSingle ? initialIds.slice(0, 1) : [...initialIds],
  );
  const [knownLabels, setKnownLabels] = useState<Record<string, string>>({});
  const contentRoot = useRef<HTMLDivElement>(null);
  // A relationship may allow several blueprints; the picker searches one at a time.
  const [targetBlueprint, setTargetBlueprint] = useState(allowedTargets[0]);
  const { isPreviewed, markPreviewed, openPreview, previewHref } =
    useRecentlyPreviewedRecords((id) => {
      setDraftIds((current) =>
        isSingle ? [id] : current.includes(id) ? current : [...current, id],
      );
      scrollRelationshipPickerToTop(contentRoot.current);
    });
  const { labelById, options, targetLabel, targets } = useRelationshipTargets(
    targetBlueprint,
    query,
    open,
  );
  const labelForId = (id: string) =>
    knownLabels[id] ?? labelById.get(id) ?? selectionLabels.get(id) ?? id;
  const availableOptions = options.filter(
    (target) => !draftIds.includes(target.id),
  );
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
    <RelationshipSelectorDialog
      actions={{
        applyLabel: t('records.applyRelationshipSelection'),
        cancelLabel: t('records.cancelRelationshipSelection'),
        clearLabel: t('records.clearRelationshipSelection'),
        onApply: () => onApply(draftIds),
        onClear: () => setDraftIds([]),
      }}
      closeLabel={t('records.closeRelationshipSelector')}
      onClose={onClose}
      open={open}
      selectedLabel={t('records.relationshipSelected', {
        count: draftIds.length,
      })}
      title={t(
        isSingle
          ? 'records.selectOneRelationship'
          : 'records.selectRelationships',
        { blueprint: allowedTargets.join(', ') },
      )}
    >
      <Stack ref={contentRoot} spacing={2}>
        {allowedTargets.length > 1 && (
          <TextField
            fullWidth
            label={t('records.relationshipTargetBlueprint')}
            onChange={(event) => setTargetBlueprint(event.target.value)}
            select
            value={targetBlueprint}
          >
            {allowedTargets.map((code) => (
              <MenuItem key={code} value={code}>
                {code}
              </MenuItem>
            ))}
          </TextField>
        )}
        <TextField
          fullWidth
          label={t('records.searchRelationshipOptions')}
          onChange={(event) => setQuery(event.target.value)}
          value={query}
        />
        <RelationshipDraftSelection
          ids={draftIds}
          labelForId={labelForId}
          onRemove={(id) =>
            setDraftIds((current) =>
              current.filter((selectedId) => selectedId !== id),
            )
          }
        />
        <Stack spacing={0.5}>
          <Typography variant="subtitle2">
            {t('records.relationshipOptions')}
          </Typography>
          <List dense disablePadding>
            {availableOptions.map((target) => (
              <RelationshipTargetOption
                isSample={target.is_sample === true}
                key={target.id}
                label={targetLabel(target)}
                onPreview={() => openPreview(target.id)}
                onPreviewLinkClick={() => markPreviewed(target.id)}
                onSelect={() => selectTarget(target.id, targetLabel(target))}
                previewHref={previewHref(target.id)}
                previewed={isPreviewed(target.id)}
              />
            ))}
          </List>
          {!targets.isPending &&
            !targets.isError &&
            availableOptions.length === 0 && (
              <Typography color="text.secondary" variant="body2">
                {t('records.noRelationshipOptions')}
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
              {t('records.loadingOptions')}
            </Typography>
          )}
          {targets.isError && (
            <Typography color="error" variant="caption">
              {t('records.couldNotLoadRecords', {
                blueprint: targetBlueprint,
              })}
            </Typography>
          )}
        </Stack>
      </Stack>
    </RelationshipSelectorDialog>
  );
};
