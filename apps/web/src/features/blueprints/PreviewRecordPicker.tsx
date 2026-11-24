import { Button, Chip, Stack, Tooltip } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { DatabaseIcon } from 'lucide-react';
import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../components/iconSizes';
import type { AttributeContext } from '../contexts/api';
import { RecordContextPicker } from '../records/components/RecordContextPicker';
import { RelationshipPickerDialog } from '../records/components/RelationshipPickerDialog';
import {
  type RelationshipPickerSelectMany,
  useRecentlyPreviewedRecords,
} from '../records/components/useRecentlyPreviewedRecords';
import { useRelationshipSelectionLabels } from '../records/components/useRelationshipSelectionLabels';
import { blueprintByCodeOptions } from '../records/queryOptions';

/**
 * Chooses an existing record of the previewed blueprint, through the same
 * Explorer picker relationship fields use, and the context its values are
 * seen from. A blueprint that has never been published has no records.
 */
export const PreviewRecordPicker = ({
  blueprintCode,
  contextId,
  contexts,
  onContextChange,
  onRecordChange,
  recordId,
}: {
  blueprintCode: string;
  contextId: string;
  contexts: readonly AttributeContext[];
  onContextChange: (contextId: string) => void;
  onRecordChange: (recordId: string | undefined) => void;
  recordId: string | undefined;
}) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  // Each opening mounts a fresh picker, starting from the chosen record.
  const [openings, setOpenings] = useState(0);
  const previewSelectRef = useRef<((id: string) => void) | null>(null);
  const previewSelectManyRef = useRef<RelationshipPickerSelectMany | null>(
    null,
  );
  const recentPreviews = useRecentlyPreviewedRecords(
    (id) => previewSelectRef.current?.(id),
    (ids, labels) => previewSelectManyRef.current?.(ids, labels),
  );
  const published = useQuery({
    ...blueprintByCodeOptions(blueprintCode),
    retry: false,
  });
  const selectedIds = recordId ? [recordId] : [];
  const labels = useRelationshipSelectionLabels(selectedIds);
  const available = Boolean(published.data);
  const choose = () => {
    setOpenings((count) => count + 1);
    setOpen(true);
  };
  const button = (
    <Button
      disabled={!available}
      onClick={choose}
      size="small"
      startIcon={<DatabaseIcon size={compactIconSize} />}
      variant="outlined"
    >
      {t(
        recordId
          ? 'blueprints.previewRecord.change'
          : 'blueprints.previewRecord.choose',
      )}
    </Button>
  );
  return (
    <Stack
      direction="row"
      spacing={1}
      sx={{ alignItems: 'center', flexWrap: 'wrap', mt: 1 }}
      useFlexGap
    >
      {recordId && (
        <Chip
          label={t('blueprints.previewRecord.showing', {
            record: labels.get(recordId) ?? recordId,
          })}
          onDelete={() => onRecordChange(undefined)}
        />
      )}
      {available || published.isPending ? (
        button
      ) : (
        <Tooltip title={t('blueprints.previewRecord.unpublished')}>
          {/* A disabled button fires no events, so the tooltip needs a host. */}
          <span>{button}</span>
        </Tooltip>
      )}
      {recordId && (
        <RecordContextPicker
          contexts={contexts}
          onChange={onContextChange}
          value={contextId}
        />
      )}
      <RelationshipPickerDialog
        allowedTargets={[blueprintCode]}
        initialIds={selectedIds}
        isSingle
        key={openings}
        onApply={([id]) => {
          onRecordChange(id);
          setOpen(false);
        }}
        onClose={() => setOpen(false)}
        open={open && available}
        previewSelectRef={previewSelectRef}
        previewSelectManyRef={previewSelectManyRef}
        recentPreviews={recentPreviews}
        selectionLabels={labels}
      />
    </Stack>
  );
};
