import {
  Button,
  ListItemIcon,
  ListItemText,
  Menu,
  MenuItem,
} from '@mui/material';
import { ChevronDownIcon } from 'lucide-react';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../components/iconSizes';
import { AgentIcon, SavedSearchIcon } from '../../components/systemIcons';

type Props = {
  disabled: boolean;
  onSaveAsSearch: () => void;
  onSendToAgent: () => void;
};

export const ExplorerSelectionActionsMenu = ({
  disabled,
  onSaveAsSearch,
  onSendToAgent,
}: Props) => {
  const { t } = useTranslation();
  const menuId = useId();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const close = () => setAnchor(null);

  return (
    <>
      <Button
        aria-controls={anchor ? menuId : undefined}
        aria-expanded={Boolean(anchor)}
        aria-haspopup="menu"
        disabled={disabled}
        endIcon={<ChevronDownIcon />}
        onClick={(event) => setAnchor(event.currentTarget)}
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
        <MenuItem
          onClick={() => {
            close();
            onSendToAgent();
          }}
        >
          <ListItemIcon>
            <AgentIcon size={compactIconSize} />
          </ListItemIcon>
          <ListItemText>{t('explorer.sendToAgentConversation')}</ListItemText>
        </MenuItem>
        <MenuItem
          onClick={() => {
            close();
            onSaveAsSearch();
          }}
        >
          <ListItemIcon>
            <SavedSearchIcon size={compactIconSize} />
          </ListItemIcon>
          <ListItemText>{t('explorer.saveSelectionAsSearch')}</ListItemText>
        </MenuItem>
      </Menu>
    </>
  );
};
