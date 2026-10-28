import { PanelRightIcon } from 'lucide-react';
import { AgentIcon } from '../../../components/systemIcons';
import { Box, IconButton, Tooltip } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { RecordToolbar } from './RecordToolbar';
import { RECORD_EXTENSION_DRAWER_ID } from '../constants';
import { RecordActionsMenu, type PanelAction } from './RecordActionsMenu';
import { RecordPublishControls } from './RecordPublishControls';
import type { RecordPublications } from './useRecordPublications';

type Props = {
  /** The record's blueprint, offered for creating another record. */
  blueprint?: { code: string; name: string };
  /** Moves the extension button into the actions menu. */
  compact?: boolean;
  recordId: string;
  extensionPanelOpen: boolean;
  onOpenExtensions: () => void;
  agentPanelOpen: boolean;
  onOpenAgent: () => void;
  schemaOutdated?: boolean;
  showExtensions: boolean;
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
};

export const RecordPreviewToolbar = ({
  blueprint,
  compact = false,
  recordId,
  extensionPanelOpen,
  onOpenExtensions,
  agentPanelOpen,
  onOpenAgent,
  schemaOutdated,
  showExtensions,
  canWrite,
  canCreate,
  canAskAgent,
  onDuplicate,
  canDelete,
  onDelete,
  canPublish,
  publications,
}: Props) => {
  const { t } = useTranslation();
  const agentAction: PanelAction | undefined =
    showExtensions && canAskAgent
      ? {
          expanded: agentPanelOpen,
          icon: AgentIcon,
          label: t('records.askAboutRecord'),
          onClick: onOpenAgent,
        }
      : undefined;
  const extensionAction: PanelAction | undefined = showExtensions
    ? {
        controls: RECORD_EXTENSION_DRAWER_ID,
        expanded: extensionPanelOpen,
        icon: PanelRightIcon,
        label: t('records.extensionContributions'),
        onClick: onOpenExtensions,
      }
    : undefined;
  // Starting a conversation stays one click away, as on the record page.
  const menuActions = compact && extensionAction ? [extensionAction] : [];
  const buttonActions = [
    agentAction,
    compact ? undefined : extensionAction,
  ].filter((action) => action !== undefined);
  return (
    <RecordToolbar label={t('records.recordPreview')}>
      <RecordActionsMenu
        blueprint={blueprint}
        canCreate={canCreate}
        canDelete={canDelete}
        canWrite={canWrite}
        onDelete={onDelete}
        onDuplicate={onDuplicate}
        panelActions={menuActions}
        recordId={recordId}
        schemaOutdated={schemaOutdated}
      />
      <Box sx={{ flexGrow: 1 }} />
      <RecordPublishControls
        canPublish={canPublish}
        publications={publications}
      />
      {buttonActions.map(
        ({ controls, expanded, icon: Icon, label, onClick }) => (
          <Tooltip key={label} title={label}>
            <IconButton
              aria-controls={controls}
              aria-expanded={expanded}
              aria-label={label}
              onClick={onClick}
            >
              <Icon />
            </IconButton>
          </Tooltip>
        ),
      )}
    </RecordToolbar>
  );
};
