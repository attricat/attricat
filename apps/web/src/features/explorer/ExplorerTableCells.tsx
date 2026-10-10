import { Box, Chip, IconButton } from '@mui/material';
import { Link } from '@tanstack/react-router';
import { EllipsisVerticalIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { RecordItem, RecordPublicationStatus } from '../records/api';
import { contextDisplayLabel } from '../records/recordDisplay';
import {
  emptyValuePlaceholder,
  recordPanelOpenerAttribute,
  publicationStatuses,
} from './constants';
import { compactIconSize } from '../../components/iconSizes';

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
}: {
  contextCodes: readonly string[];
  record: RecordItem;
  onOpenPanel?: OpenRecordPanel;
}) => {
  const { t } = useTranslation();
  return (
    <Box sx={{ alignItems: 'center', display: 'flex', gap: 1 }}>
      <Link
        {...(onOpenPanel && { [recordPanelOpenerAttribute]: true })}
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

export const PublicationStatusCell = ({
  publication,
}: {
  publication: RecordPublicationStatus | undefined;
}) => {
  const { t } = useTranslation();
  if (!publication) return emptyValuePlaceholder;
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
