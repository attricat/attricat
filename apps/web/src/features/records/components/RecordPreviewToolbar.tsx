import type { ReactNode } from 'react';
import { AgentIcon } from '../../../components/systemIcons';
import { Box, IconButton, Tooltip } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { RecordToolbar } from './RecordToolbar';
import { RecordActionsMenu } from './RecordActionsMenu';
import { RecordPublishControls } from './RecordPublishControls';
import type { RecordPublications } from './useRecordPublications';

type Props = {
  /** The record's blueprint, offered for creating another record. */
  blueprint?: { code: string; name: string };
  recordId: string;
  agentPanelOpen: boolean;
  onOpenAgent: () => void;
  schemaOutdated?: boolean;
  /** The record has loaded, so an agent can be asked about it. */
  loaded: boolean;
  /** The caller may change this record (duplicate it or upgrade its blueprint). */
  canWrite: boolean;
  /** The caller may create records in the workspace. */
  canCreate: boolean;
  /** The caller may ask an agent about the record. */
  canAskAgent: boolean;
  onDuplicate: () => void;
  canDelete: boolean;
  onDelete: () => void;
  canPublish: boolean;
  publications: RecordPublications;
  /** Extension header actions, offered in the actions menu. */
  extensionActions?: ReactNode;
};

export const RecordPreviewToolbar = ({
  blueprint,
  recordId,
  agentPanelOpen,
  onOpenAgent,
  schemaOutdated,
  loaded,
  canWrite,
  canCreate,
  canAskAgent,
  onDuplicate,
  canDelete,
  onDelete,
  canPublish,
  publications,
  extensionActions,
}: Props) => {
  const { t } = useTranslation();
  const askLabel = t('records.askAboutRecord');
  return (
    <RecordToolbar label={t('records.recordPreview')}>
      <RecordActionsMenu
        blueprint={blueprint}
        canCreate={canCreate}
        canDelete={canDelete}
        canWrite={canWrite}
        extensionActions={extensionActions}
        onDelete={onDelete}
        onDuplicate={onDuplicate}
        publications={canPublish ? publications : undefined}
        recordId={recordId}
        schemaOutdated={schemaOutdated}
      />
      <Box sx={{ flexGrow: 1 }} />
      <RecordPublishControls
        canPublish={canPublish}
        publications={publications}
      />
      {loaded && canAskAgent && (
        <Tooltip title={askLabel}>
          <IconButton
            aria-expanded={agentPanelOpen}
            aria-label={askLabel}
            onClick={onOpenAgent}
          >
            <AgentIcon />
          </IconButton>
        </Tooltip>
      )}
    </RecordToolbar>
  );
};
