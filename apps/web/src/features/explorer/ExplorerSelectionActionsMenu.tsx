import { Button, Menu } from '@mui/material';
import { ChevronDownIcon } from 'lucide-react';
import { useId, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { AgentIcon, SavedSearchIcon } from '../../components/systemIcons';
import { useCan } from '../auth/useCan';
import { capabilities } from '../auth/capabilities';
import { useActionDialogStore } from '../extensions/actionDialogStore';
import { ActionMenuCloseContext } from '../extensions/actions';
import { ActionMenuItem } from '../../components/ActionMenuItem';

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
  const menuId = useId();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const close = () => setAnchor(null);
  // An extension action that opens the host dialog hands off to it; the
  // dialog keeps its captured selection after this menu and its frames close.
  const dialogKey = useActionDialogStore((state) => state.dialog?.key);
  const [openedDialogKey, setOpenedDialogKey] = useState(dialogKey);
  if (anchor && dialogKey !== undefined && dialogKey !== openedDialogKey) {
    setOpenedDialogKey(dialogKey);
    setAnchor(null);
  }

  return (
    <>
      <Button
        aria-controls={anchor ? menuId : undefined}
        aria-expanded={Boolean(anchor)}
        aria-haspopup="menu"
        disabled={disabled}
        endIcon={<ChevronDownIcon />}
        onClick={(event) => {
          setOpenedDialogKey(dialogKey);
          setAnchor(event.currentTarget);
        }}
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
        {canSendToAgent && (
          <ActionMenuItem
            icon={AgentIcon}
            onClick={() => {
              close();
              onSendToAgent();
            }}
          >
            {t('explorer.sendToAgentConversation')}
          </ActionMenuItem>
        )}
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
