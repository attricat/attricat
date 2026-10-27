import { Box, Chip, IconButton } from '@mui/material';
import { Link } from '@tanstack/react-router';
import { EllipsisVerticalIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import type { RecordItem } from '../records/api';
import { recordPublicationOptions } from '../records/queryOptions';
import { contextDisplayLabel } from '../records/recordDisplay';
import { recordPanelOpenerAttribute } from './constants';
import { compactIconSize } from '../../components/iconSizes';
import { emptyValuePlaceholder } from '../../components/placeholders';
import { publicationStatuses } from '../records/constants';
import { SystemTagChips } from '../records/components/SystemTagChips';
import { visibleSystemTags } from '../records/systemTags';

export type ActionMenuPosition = { left: number; top: number };

export type OpenRecordPanel = (recordId: string, opener: HTMLElement) => void;

/**
 * Links to the record page. With `onOpenPanel`, a plain click opens the
 * record in a panel over the results instead; modified clicks still open the page.
 */
export const RecordDisplayCell = ({
  contextCodes,
  record,
  onOpenPanel,
  onPrefetchPanel,
}: {
  contextCodes: readonly string[];
  record: RecordItem;
  onOpenPanel?: OpenRecordPanel;
  /** Starts loading the panel when the link is pointed at or focused. */
  onPrefetchPanel?: (record: RecordItem) => void;
}) => {
  const { t } = useTranslation();
  const prefetch = onPrefetchPanel && (() => onPrefetchPanel(record));
  return (
    <Box sx={{ alignItems: 'center', display: 'flex', gap: 1 }}>
      <Link
        {...(onOpenPanel && { [recordPanelOpenerAttribute]: true })}
        onFocus={prefetch}
        onPointerEnter={prefetch}
        onClick={(event) => {
          if (
            !onOpenPanel ||
            event.button !== 0 ||
            event.altKey ||
            event.ctrlKey ||
            event.metaKey ||
            event.shiftKey
          )
            return;
          event.preventDefault();
          onOpenPanel(record.id, event.currentTarget);
        }}
        params={{ recordId: record.id }}
        to="/records/$recordId"
      >
        {contextDisplayLabel(record.display, record.id, contextCodes)}
      </Link>
      {record.is_sample && (
        <Chip color="info" label={t('records.sample')} size="small" />
      )}
    </Box>
  );
};

/**
 * The record's status in the publication context. The table loads the
 * statuses of its visible rows together; each cell only follows its own, so a
 * status that arrives re-renders one cell.
 */
export const PublicationStatusCell = ({
  contextId,
  recordId,
}: {
  contextId: string | undefined;
  recordId: string;
}) => {
  const { t } = useTranslation();
  const { data: publication } = useQuery({
    ...recordPublicationOptions(recordId),
    enabled: false,
    select: (publications) =>
      publications.find((item) => item.context_id === contextId),
  });
  if (!contextId || !publication) return emptyValuePlaceholder;
  return (
    <Chip
      color={
        publication.status === publicationStatuses.published
          ? 'success'
          : 'default'
      }
      label={t(`records.publication.${publication.status}`)}
      size="small"
    />
  );
};

export const SchemaVersionCell = ({ record }: { record: RecordItem }) => {
  const { t } = useTranslation();
  return (
    <Chip
      color={record.schema_outdated ? 'warning' : 'success'}
      label={t('explorer.schemaVersionStatus', {
        status: record.schema_outdated
          ? t('explorer.outdated')
          : t('explorer.current'),
        version: record.blueprint_version,
      })}
      size="small"
    />
  );
};

export const SystemTagsCell = ({
  record,
  onFilterTag,
}: {
  record: RecordItem;
  /** Filters the results by a clicked tag. */
  onFilterTag?: (tag: string) => void;
}) => {
  const { t } = useTranslation();
  return visibleSystemTags(record.system_tags).length ? (
    <SystemTagChips
      onSelect={onFilterTag}
      selectLabel={(tag) => t('explorer.filterBySystemTag', { tag })}
      tags={record.system_tags}
    />
  ) : (
    emptyValuePlaceholder
  );
};

export const RecordActionsCell = ({
  recordId,
  onOpenActions,
}: {
  recordId: string;
  onOpenActions: (recordId: string, position: ActionMenuPosition) => void;
}) => {
  const { t } = useTranslation();
  return (
    <IconButton
      aria-label={t('explorer.recordActionsFor', { recordId })}
      onClick={(event) => {
        const { left, top } = event.currentTarget.getBoundingClientRect();
        onOpenActions(recordId, { left, top });
      }}
      size="small"
    >
      <EllipsisVerticalIcon size={compactIconSize} />
    </IconButton>
  );
};
