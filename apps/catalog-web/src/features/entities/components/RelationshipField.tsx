import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Button,
  FormControl,
  List,
  MenuItem,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { compactOutlinedActionButtonSx } from '../../../components/CompactOutlinedActionButton';
import { LoadMoreButton } from '../../../components/LoadMoreButton';
import { RelationshipSelectorDialog } from '../../../components/RelationshipSelectorDialog';
import { RelationshipPickerIcon } from '../../../components/systemIcons';
import type { Attribute } from '../api';
import { attributeCardinalities, RELATIONSHIP_ID_JOINER } from '../constants';
import { allowedTargetBlueprints, relationshipIdsForField } from '../entityForm';
import { attributeLabel } from '../entityDisplay';
import { RelationshipDraftSelection } from './RelationshipDraftSelection';
import { RelationshipSelectionPills } from './RelationshipSelectionPills';
import { RelationshipTargetOption } from './RelationshipTargetOption';
import { useRelationshipSelectionLabels } from './useRelationshipSelectionLabels';
import { useRelationshipTargets } from './useRelationshipTargets';
import {
  scrollRelationshipPickerToTop,
  useRecentlyPreviewedEntities,
} from './useRecentlyPreviewedEntities';
import { smallIconSize } from '../../../components/iconSizes';

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
  const allowedTargets = allowedTargetBlueprints(attribute);
  const [activeTarget, setActiveTarget] = useState<string>();
  // A relationship may allow several blueprints; the picker searches one at a time.
  const targetBlueprint =
    activeTarget && allowedTargets.includes(activeTarget)
      ? activeTarget
      : allowedTargets[0];
  const selectedIds = relationshipIdsForField(value);
  const isSingle = attribute.cardinality === attributeCardinalities.one;
  const selectorOpen = open && !disabled;
  const selectionLabels = useRelationshipSelectionLabels(
    allowedTargets,
    selectedIds,
  );
  const { isPreviewed, markPreviewed, openPreview, previewHref } =
    useRecentlyPreviewedEntities((id) => {
      setDraftIds((current) =>
        isSingle ? [id] : current.includes(id) ? current : [...current, id],
      );
      scrollRelationshipPickerToTop(pickerContentRoot.current);
    });
  const { labelById, options, targetLabel, targets } = useRelationshipTargets(
    targetBlueprint,
    query,
    selectorOpen,
  );

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

  const labelForId = (id: string) =>
    knownLabels[id] ?? labelById.get(id) ?? selectionLabels.get(id) ?? id;
  const availableOptions = options.filter(
    (target) => !draftIds.includes(target.id),
  );
  const removeDraftId = (id: string) =>
    setDraftIds((current) => current.filter((selectedId) => selectedId !== id));
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
  const applySelection = () => {
    if (!disabled) onChange(draftIds.join(RELATIONSHIP_ID_JOINER));
    setOpen(false);
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
                startIcon={<RelationshipPickerIcon size={smallIconSize} />}
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
                        .join(RELATIONSHIP_ID_JOINER),
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
          onApply: applySelection,
          onClear: () => setDraftIds([]),
        }}
        closeLabel={t('entities.closeRelationshipSelector')}
        onClose={() => setOpen(false)}
        open={selectorOpen}
        selectedLabel={t('entities.relationshipSelected', {
          count: draftIds.length,
        })}
        title={t(
          isSingle
            ? 'entities.selectOneRelationship'
            : 'entities.selectRelationships',
          { blueprint: allowedTargets.join(', ') },
        )}
      >
        <Stack ref={pickerContentRoot} spacing={2}>
          {allowedTargets.length > 1 && (
            <TextField
              fullWidth
              label={t('entities.relationshipTargetBlueprint')}
              onChange={(event) => setActiveTarget(event.target.value)}
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
            label={t('entities.searchRelationshipOptions')}
            onChange={(event) => setQuery(event.target.value)}
            value={query}
          />
          <RelationshipDraftSelection
            ids={draftIds}
            labelForId={labelForId}
            onRemove={removeDraftId}
          />
          <Stack spacing={0.5}>
            <Typography variant="subtitle2">
              {t('entities.relationshipOptions')}
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
