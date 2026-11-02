import { Button, Menu, Tooltip } from '@mui/material';
import { ChevronDownIcon } from 'lucide-react';
import { useId, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { AgentIcon, SavedSearchIcon } from '../../components/systemIcons';
import { useAgentsUnavailableReason } from '../agents/useAgentsUnavailableReason';
import { useCan } from '../auth/useCan';
import { capabilities } from '../auth/capabilities';
import { ActionMenuCloseContext } from '../extensions/actions';
import { ActionMenuItem } from '../../components/ActionMenuItem';
import { useActionMenuAnchor } from '../extensions/useActionMenuAnchor';

type Props = {
  disabled: boolean;
  /** Extension actions for the selection, rendered as menu items. */
  extensionActions?: ReactNode;
  onSaveAsSearch: () => void;
  onSendToAgent: () => void;
};

export const ExplorerSelectionActionsMenu = ({
  disabled,
  extensionActions,
  onSaveAsSearch,
  onSendToAgent,
}: Props) => {
  const { t } = useTranslation();
  const canSendToAgent = useCan(capabilities.agentsRun);
  const agentsUnavailableReason = useAgentsUnavailableReason();
  const menuId = useId();
  const { anchor, close, open } = useActionMenuAnchor();

  return (
    <>
      <Button
        aria-controls={anchor ? menuId : undefined}
        aria-expanded={Boolean(anchor)}
        aria-haspopup="menu"
        disabled={disabled}
        endIcon={<ChevronDownIcon />}
        onClick={open}
        size="small"
        variant="contained"
      >
        {t('explorer.selectionActions')}
      </Button>
      <Menu
        anchorEl={anchor}
        anchorOrigin={{ horizontal: 'right', vertical: 'bottom' }}
        id={menuId}
        onClose={close}
        open={Boolean(anchor)}
        transformOrigin={{ horizontal: 'right', vertical: 'top' }}
      >
        {canSendToAgent &&
          (agentsUnavailableReason ? (
            // A wrapper would hide an enabled item from arrow-key navigation.
            <Tooltip placement="left" title={agentsUnavailableReason}>
              <span>
                <ActionMenuItem disabled icon={AgentIcon}>
                  {t('explorer.sendToAgentConversation')}
                </ActionMenuItem>
              </span>
            </Tooltip>
          ) : (
            <ActionMenuItem
              icon={AgentIcon}
              onClick={() => {
                close();
                onSendToAgent();
              }}
            >
              {t('explorer.sendToAgentConversation')}
            </ActionMenuItem>
          ))}
        <ActionMenuItem
          icon={SavedSearchIcon}
          onClick={() => {
            close();
            onSaveAsSearch();
          }}
        >
          {t('explorer.saveSelectionAsSearch')}
        </ActionMenuItem>
        <ActionMenuCloseContext value={close}>
          {extensionActions}
        </ActionMenuCloseContext>
      </Menu>
    </>
  );
};
