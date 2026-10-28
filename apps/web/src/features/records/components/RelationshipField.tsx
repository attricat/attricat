import { forwardRef, useId, useRef, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import {
  Box,
  Button,
  InputAdornment,
  Stack,
  TextField,
  type InputBaseComponentProps,
  type SxProps,
  type Theme,
} from '@mui/material';
import { compactOutlinedActionButtonSx } from '../../../components/CompactOutlinedActionButton';
import {
  RelationshipIcon,
  RelationshipPickerIcon,
} from '../../../components/systemIcons';
import type { Attribute } from '../api';
import { attributeCardinalities, RELATIONSHIP_ID_JOINER } from '../constants';
import { recordBlueprintsOptions } from '../queryOptions';
import {
  allowedTargetBlueprints,
  relationshipIdsForField,
} from '../recordForm';
import { useRelatedRecordPreview } from '../linkedRecord';
import { attributeLabel } from '../recordDisplay';
import {
  RelatedRecordPreviewDetails,
  RelatedRecordPreviewToggle,
} from './RelatedRecordPreview';
import { RelationshipPickerDialog } from './RelationshipPickerDialog';
import { useRecentlyPreviewedRecords } from './useRecentlyPreviewedRecords';
import { RelationshipSelectionPills } from './RelationshipSelectionPills';
import { useRelationshipSelectionLabels } from './useRelationshipSelectionLabels';
import { smallIconSize } from '../../../components/iconSizes';
import { FieldHint } from '../../../components/FieldHint';

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
  // Each opening mounts a fresh picker, starting from the field's value.
  const [openings, setOpenings] = useState(0);
  // Kept across openings, so a preview tab opened earlier can still select.
  const previewSelectRef = useRef<((id: string) => void) | null>(null);
  const recentPreviews = useRecentlyPreviewedRecords((id) =>
    previewSelectRef.current?.(id),
  );
  const restrictedTargets = allowedTargetBlueprints(attribute);
  // A relationship that names no target blueprint may link to any of them.
  const anyBlueprint = restrictedTargets.length === 0;
  const recordBlueprints = useQuery({
    ...recordBlueprintsOptions(),
    enabled: anyBlueprint,
  });
  const allowedTargets = anyBlueprint
    ? (recordBlueprints.data ?? []).map((blueprint) => blueprint.code)
    : restrictedTargets;
  const selectedIds = relationshipIdsForField(value);
  const isSingle = attribute.cardinality === attributeCardinalities.one;
  const selectorDisabled = disabled || allowedTargets.length === 0;
  const selectorOpen = open && !selectorDisabled;
  const selectionLabels = useRelationshipSelectionLabels(selectedIds);
  const preview = useRelatedRecordPreview(
    attribute,
    selectedIds.length === 1 ? selectedIds[0] : undefined,
  );

  const previewLabel = preview
    ? (selectionLabels.get(preview.recordId) ?? preview.recordId)
    : '';
  const openSelector = () => {
    setOpenings((count) => count + 1);
    setOpen(true);
  };

  return (
    <>
      <TextField
        error={Boolean(error)}
        disabled={disabled}
        fullWidth
        helperText={
          error ?? (helperText && <FieldHint>{helperText}</FieldHint>)
        }
        id={fieldId}
        label={attributeLabel(attribute)}
        required={required}
        slotProps={{
          htmlInput: {
            'aria-labelledby': `${fieldId}-label`,
            children: (
              <Stack sx={{ minWidth: 0 }}>
                <RelationshipSelectionPills
                  action={
                    <>
                      <Button
                        aria-label={t('records.openRelationshipSelectorFor', {
                          field: attributeLabel(attribute),
                        })}
                        color="primary"
                        disabled={selectorDisabled}
                        onClick={openSelector}
                        size="small"
                        startIcon={
                          <RelationshipPickerIcon size={smallIconSize} />
                        }
                        sx={compactOutlinedActionButtonSx}
                        variant="outlined"
                      >
                        {t('records.openRelationshipSelector')}
                      </Button>
                      {preview && (
                        <RelatedRecordPreviewToggle
                          label={previewLabel}
                          preview={preview}
                        />
                      )}
                    </>
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
                {preview && (
                  <RelatedRecordPreviewDetails
                    label={previewLabel}
                    preview={preview}
                  />
                )}
              </Stack>
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
      <RelationshipPickerDialog
        allowedTargets={allowedTargets}
        anyBlueprint={anyBlueprint}
        initialIds={selectedIds}
        isSingle={isSingle}
        key={openings}
        onApply={(ids) => {
          if (!disabled) onChange(ids.join(RELATIONSHIP_ID_JOINER));
          setOpen(false);
        }}
        onClose={() => setOpen(false)}
        open={selectorOpen}
        previewSelectRef={previewSelectRef}
        recentPreviews={recentPreviews}
        selectionLabels={selectionLabels}
      />
    </>
  );
};
