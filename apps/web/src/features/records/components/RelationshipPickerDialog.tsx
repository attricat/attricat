import { useEffect, useId, useRef, useState, type RefObject } from 'react';
import type { RelationshipTargetFilter } from './relationshipScope';
import { useTranslation } from 'react-i18next';
import { Stack, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { EmptyState } from '../../../components/EmptyState';
import { LoadMoreButton } from '../../../components/LoadMoreButton';
import { RelationshipSelectorDialog } from '../../../components/RelationshipSelectorDialog';
import { RecordIcon } from '../../../components/systemIcons';
import { relationshipPickerModes } from '../constants';
import { lexiconText } from '../../lexicon/lexicon';
import { recordBlueprintsOptions } from '../queryOptions';
import { RelationshipDraftSelection } from './RelationshipDraftSelection';
import { useLatestCallback } from '../../../components/useLatestCallback';
import {
  RelationshipTargetList,
  RelationshipTargetOption,
} from './RelationshipTargetOption';
import { useRelationshipTargets } from './useRelationshipTargets';
import {
  scrollRelationshipPickerToTop,
  type RecentlyPreviewedRecords,
  type RelationshipPickerSelectMany,
} from './useRecentlyPreviewedRecords';
import {
  RelationshipTargetSearchRow,
  TargetBlueprintSelect,
} from './RelationshipTargetSearch';
import { relationshipPickerExplorerHref } from '../relationshipPickerHref';

/**
 * Picks the records a relationship field links to. Mount it with a new `key`
 * for each opening: the draft selection, search and target start from
 * `initialIds` and are only handed back through `onApply`. Previewed records
 * come from the field, so a record previewed in an earlier opening stays
 * marked and can still be selected from its tab.
 */
export const RelationshipPickerDialog = ({
  allowedTargets,
  anyBlueprint = false,
  initialIds,
  isSingle,
  onApply,
  onClose,
  open,
  previewSelectRef,
  previewSelectManyRef,
  recentPreviews,
  selectionLabels,
  targetFilters,
}: {
  /** Blueprints the relationship may link to; the first is searched first. */
  allowedTargets: readonly string[];
  /** The relationship names no target, so `allowedTargets` is every blueprint. */
  anyBlueprint?: boolean;
  initialIds: readonly string[];
  isSingle: boolean;
  onApply: (ids: string[]) => void;
  onClose: () => void;
  open: boolean;
  /** Where the picker takes a record selected from its preview tab. */
  previewSelectRef: RefObject<((id: string) => void) | null>;
  /** Where the picker takes records chosen in an Explorer tab it opened. */
  previewSelectManyRef: RefObject<RelationshipPickerSelectMany | null>;
  recentPreviews: RecentlyPreviewedRecords;
  /** Labels of the records already selected in the field. */
  selectionLabels: ReadonlyMap<string, string>;
  /** Narrows the offered targets, such as a relationship's `target_scope`. */
  targetFilters?: readonly RelationshipTargetFilter[];
}) => {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [draftIds, setDraftIds] = useState<string[]>(() =>
    isSingle ? initialIds.slice(0, 1) : [...initialIds],
  );
  const [knownLabels, setKnownLabels] = useState<Record<string, string>>({});
  const contentRoot = useRef<HTMLDivElement>(null);
  const optionName = useId();
  // A relationship may allow several blueprints; the picker searches one at a time.
  const [targetBlueprint, setTargetBlueprint] = useState(allowedTargets[0]);
  const { isPreviewed, markPreviewed, pickerToken, previewHref } =
    recentPreviews;
  const { labelById, options, targetLabel, targets } = useRelationshipTargets(
    targetBlueprint,
    query,
    open,
    targetFilters,
  );
  const refetchTargets = useLatestCallback(() => targets.refetch());
  useEffect(() => {
    previewSelectRef.current = (id) => {
      setDraftIds((current) =>
        isSingle ? [id] : current.includes(id) ? current : [...current, id],
      );
      scrollRelationshipPickerToTop(contentRoot.current);
    };
    return () => {
      previewSelectRef.current = null;
    };
  }, [isSingle, previewSelectRef]);
  useEffect(() => {
    previewSelectManyRef.current = (ids, labels) => {
      if (ids.length === 0) return;
      // Records created in the Explorer tab are not in the loaded options.
      void refetchTargets();
      setKnownLabels((current) => ({ ...current, ...labels }));
      setDraftIds((current) =>
        isSingle
          ? ids.slice(0, 1)
          : [...current, ...ids.filter((id) => !current.includes(id))],
      );
      scrollRelationshipPickerToTop(contentRoot.current);
    };
    return () => {
      previewSelectManyRef.current = null;
    };
  }, [isSingle, previewSelectManyRef, refetchTargets]);

  const blueprints = useQuery({ ...recordBlueprintsOptions(), enabled: open });
  const explorerHref = targetBlueprint
    ? relationshipPickerExplorerHref({
        blueprint: targetBlueprint,
        mode: isSingle
          ? relationshipPickerModes.one
          : relationshipPickerModes.many,
        targetFilters,
        token: pickerToken,
      })
    : undefined;
  const blueprintName = (code: string) => {
    const name = blueprints.data?.find(
      (blueprint) => blueprint.code === code,
    )?.name;
    return name ? lexiconText(name) : code;
  };
  const labelForId = (id: string) =>
    knownLabels[id] ?? labelById.get(id) ?? selectionLabels.get(id) ?? id;
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
      title={
        anyBlueprint
          ? t(
              isSingle
                ? 'records.selectOneAnyRelationship'
                : 'records.selectAnyRelationships',
            )
          : t(
              isSingle
                ? 'records.selectOneRelationship'
                : 'records.selectRelationships',
              { blueprint: allowedTargets.map(blueprintName).join(', ') },
            )
      }
    >
      <Stack ref={contentRoot} spacing={2}>
        <TargetBlueprintSelect
          blueprintName={blueprintName}
          onChange={setTargetBlueprint}
          targets={allowedTargets}
          value={targetBlueprint}
        />
        <RelationshipTargetSearchRow
          explorerHref={explorerHref}
          onQueryChange={setQuery}
          query={query}
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
          <RelationshipTargetList
            isSingle={isSingle}
            label={t('records.relationshipOptions')}
          >
            {options.map((target) => (
              <RelationshipTargetOption
                isSample={target.is_sample === true}
                isSingle={isSingle}
                key={target.id}
                label={targetLabel(target)}
                name={optionName}
                onPreview={() => markPreviewed(target.id)}
                onSelect={() => selectTarget(target.id, targetLabel(target))}
                previewHref={previewHref(target.id)}
                previewed={isPreviewed(target.id)}
                selected={draftIds.includes(target.id)}
              />
            ))}
          </RelationshipTargetList>
          {!targets.isPending && !targets.isError && options.length === 0 && (
            <EmptyState
              icon={RecordIcon}
              title={t('records.noRelationshipOptions')}
            />
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
                blueprint: blueprintName(targetBlueprint),
              })}
            </Typography>
          )}
        </Stack>
      </Stack>
    </RelationshipSelectorDialog>
  );
};
