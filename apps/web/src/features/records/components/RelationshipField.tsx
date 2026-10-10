import { forwardRef, useId, useRef, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Box,
  Button,
  InputAdornment,
  List,
  MenuItem,
  Stack,
  TextField,
  Typography,
  type InputBaseComponentProps,
  type SxProps,
  type Theme,
} from '@mui/material';
import { compactOutlinedActionButtonSx } from '../../../components/CompactOutlinedActionButton';
import { LoadMoreButton } from '../../../components/LoadMoreButton';
import { RelationshipSelectorDialog } from '../../../components/RelationshipSelectorDialog';
import {
  RelationshipIcon,
  RelationshipPickerIcon,
} from '../../../components/systemIcons';
import type { Attribute } from '../api';
import { attributeCardinalities, RELATIONSHIP_ID_JOINER } from '../constants';
import {
  allowedTargetBlueprints,
  relationshipIdsForField,
} from '../recordForm';
import { attributeLabel } from '../recordDisplay';
import { RelationshipDraftSelection } from './RelationshipDraftSelection';
import { RelationshipSelectionPills } from './RelationshipSelectionPills';
import { RelationshipTargetOption } from './RelationshipTargetOption';
import { useRelationshipSelectionLabels } from './useRelationshipSelectionLabels';
import { useRelationshipTargets } from './useRelationshipTargets';
import {
  scrollRelationshipPickerToTop,
  useRecentlyPreviewedRecords,
} from './useRecentlyPreviewedRecords';
import { smallIconSize } from '../../../components/iconSizes';

const relationshipAdornment = (
  <InputAdornment position="start">
    <RelationshipIcon size={smallIconSize} />
  </InputAdornment>
);

/** Height of a pill and of the selector button beside the pills. */
const pillRowHeight = '24px';
/**
 * Padding that keeps one row of pills as tall as a small text field's input
 * (MUI gives it 8.5px padding around a 1.4375em line).
 */
const pillRowPadding = `calc(8.5px - (${pillRowHeight} - 1.4375em) / 2)`;

/** Lets the pills grow the outlined field over several rows. */
const relationshipInputSx = {
  alignItems: 'flex-start',
  '& .MuiInputAdornment-root': {
    alignSelf: 'flex-start',
    height: pillRowHeight,
    maxHeight: 'none',
    mt: pillRowPadding,
  },
  '& .MuiInputBase-input': { height: 'auto', py: pillRowPadding },
} satisfies SxProps<Theme>;

/**
 * Stands in for the input of an outlined text field so the selected pills get
 * its label, border, and focus, error, and disabled states. Focus moving
 * between the pills and the selector button reads as focus on the field.
 */
const RelationshipFieldInput = forwardRef<
  HTMLDivElement,
  InputBaseComponentProps
>(
  (
    {
      'aria-describedby': describedBy,
      'aria-labelledby': labelledBy,
      children,
      className,
      onBlur,
      onFocus,
    },
    ref,
  ) => (
    <Box
      aria-describedby={describedBy}
      aria-labelledby={labelledBy}
      className={className}
      onBlur={onBlur}
      onFocus={onFocus}
      ref={ref}
      role="group"
    >
      {children as ReactNode}
    </Box>
  ),
);

export const RelationshipField = ({
  attribute,
  disabled = false,
  error,
  helperText,
  linkToRecords = false,
  onChange,
  required,
  value,
}: {
  attribute: Attribute;
  disabled?: boolean;
  required?: boolean;
  error?: string;
  helperText?: string;
  /** Lets each selected record be opened from its pill. */
  linkToRecords?: boolean;
  onChange: (value: string) => void;
  value: string;
}) => {
  const { t } = useTranslation();
  const fieldId = useId();
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
    useRecentlyPreviewedRecords((id) => {
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
        required={required}
        helperText={error ?? t('records.commaSeparatedUuids')}
        onChange={(event) => onChange(event.target.value)}
        slotProps={{ input: { startAdornment: relationshipAdornment } }}
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
    <>
      <TextField
        error={Boolean(error)}
        disabled={disabled}
        fullWidth
        helperText={error ?? helperText}
        id={fieldId}
        label={attributeLabel(attribute)}
        required={required}
        slotProps={{
          htmlInput: {
            'aria-labelledby': `${fieldId}-label`,
            children: (
              <RelationshipSelectionPills
                action={
                  <Button
                    aria-label={t('records.openRelationshipSelectorFor', {
                      field: attributeLabel(attribute),
                    })}
                    color="primary"
                    disabled={disabled}
                    onClick={openSelector}
                    size="small"
                    startIcon={<RelationshipPickerIcon size={smallIconSize} />}
                    sx={compactOutlinedActionButtonSx}
                    variant="outlined"
                  >
                    {t('records.openRelationshipSelector')}
                  </Button>
                }
                gap={1}
                ids={selectedIds}
                labels={selectionLabels}
                linkToRecords={linkToRecords}
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
            ),
          },
          input: {
            inputComponent: RelationshipFieldInput,
            startAdornment: relationshipAdornment,
            sx: relationshipInputSx,
          },
          inputLabel: { shrink: true },
        }}
      />
      <RelationshipSelectorDialog
        actions={{
          applyLabel: t('records.applyRelationshipSelection'),
          cancelLabel: t('records.cancelRelationshipSelection'),
          clearLabel: t('records.clearRelationshipSelection'),
          onApply: applySelection,
          onClear: () => setDraftIds([]),
        }}
        closeLabel={t('records.closeRelationshipSelector')}
        onClose={() => setOpen(false)}
        open={selectorOpen}
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
        <Stack ref={pickerContentRoot} spacing={2}>
          {allowedTargets.length > 1 && (
            <TextField
              fullWidth
              label={t('records.relationshipTargetBlueprint')}
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
            label={t('records.searchRelationshipOptions')}
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
    </>
  );
};
