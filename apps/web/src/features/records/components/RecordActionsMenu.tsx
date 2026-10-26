import {
  ChevronDownIcon,
  CircleArrowUpIcon,
  CopyIcon,
  HashIcon,
  PlusIcon,
  RotateCcwClockIcon,
  Trash2Icon,
  type LucideIcon,
} from 'lucide-react';
import {
  Button,
  Divider,
  ListItemIcon,
  ListItemText,
  Menu,
  MenuItem,
} from '@mui/material';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { RouterMenuItem } from '../../../components/RouterLink';
import { useCopyToClipboard } from '../../../components/useCopyToClipboard';
import { compactIconSize } from '../../../components/iconSizes';
import { lexiconText } from '../../lexicon/lexicon';

/** A button that opens one of the record's side panels. */
export type PanelAction = {
  /** The id of the panel it opens, when the panel is always mounted. */
  controls?: string;
  expanded: boolean;
  icon: LucideIcon;
  label: string;
  onClick: () => void;
};

/** The record's Actions menu: history, copying, creating and deleting. */
export const RecordActionsMenu = ({
  blueprint,
  canCreate,
  canDelete,
  canWrite,
  onDelete,
  onDuplicate,
  panelActions,
  recordId,
  schemaOutdated,
}: {
  /** The record's blueprint, offered for creating another record. */
  blueprint?: { code: string; name: string };
  /** The caller may create records in the workspace. */
  canCreate: boolean;
  canDelete: boolean;
  /** The caller may change this record (duplicate it or upgrade its blueprint). */
  canWrite: boolean;
  onDelete: () => void;
  onDuplicate: () => void;
  /** Panels opened from the menu rather than from the toolbar. */
  panelActions: readonly PanelAction[];
  recordId: string;
  schemaOutdated?: boolean;
}) => {
  const { t } = useTranslation();
  const menuId = useId();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const close = () => setAnchor(null);
  const copyText = useCopyToClipboard();
  const choose = (action: () => void) => () => {
    close();
    action();
  };
  return (
    <>
      <Button
        aria-controls={anchor ? menuId : undefined}
        aria-expanded={Boolean(anchor)}
        aria-haspopup="menu"
        color="inherit"
        endIcon={<ChevronDownIcon />}
        onClick={(event) => setAnchor(event.currentTarget)}
        size="small"
      >
        {t('records.recordActions')}
      </Button>
      <Menu
        anchorEl={anchor}
        id={menuId}
        onClose={close}
        open={Boolean(anchor)}
      >
        <RouterMenuItem
          onClick={close}
          params={{ recordId }}
          to="/records/$recordId/changes"
        >
          <ListItemIcon>
            <RotateCcwClockIcon size={compactIconSize} />
          </ListItemIcon>
          <ListItemText>{t('records.changes')}</ListItemText>
        </RouterMenuItem>
        <MenuItem onClick={choose(() => void copyText(recordId))}>
          <ListItemIcon>
            <HashIcon size={compactIconSize} />
          </ListItemIcon>
          <ListItemText>{t('records.copyRecordId')}</ListItemText>
        </MenuItem>
        {canWrite && (
          <MenuItem onClick={choose(onDuplicate)}>
            <ListItemIcon>
              <CopyIcon size={compactIconSize} />
            </ListItemIcon>
            <ListItemText>{t('records.duplicateRecord')}</ListItemText>
          </MenuItem>
        )}
        {canCreate && blueprint && (
          <RouterMenuItem
            onClick={close}
            search={{ blueprint: blueprint.code }}
            to="/records/new"
          >
            <ListItemIcon>
              <PlusIcon size={compactIconSize} />
            </ListItemIcon>
            <ListItemText>
              {t('records.createAnother', {
                blueprint: lexiconText(blueprint.name),
              })}
            </ListItemText>
          </RouterMenuItem>
        )}
        {canWrite && schemaOutdated && (
          <RouterMenuItem
            onClick={close}
            params={{ recordId }}
            to="/records/$recordId/migrate"
          >
            <ListItemIcon>
              <CircleArrowUpIcon size={compactIconSize} />
            </ListItemIcon>
            <ListItemText>{t('records.upgradeBlueprint')}</ListItemText>
          </RouterMenuItem>
        )}
        {panelActions.length > 0 && <Divider />}
        {panelActions.map(({ controls, icon: Icon, label, onClick }) => (
          <MenuItem
            aria-controls={controls}
            key={label}
            onClick={choose(onClick)}
          >
            <ListItemIcon>
              <Icon size={compactIconSize} />
            </ListItemIcon>
            <ListItemText>{label}</ListItemText>
          </MenuItem>
        ))}
        {canDelete && <Divider />}
        {canDelete && (
          <MenuItem onClick={choose(onDelete)} sx={{ color: 'error.main' }}>
            <ListItemIcon sx={{ color: 'inherit' }}>
              <Trash2Icon size={compactIconSize} />
            </ListItemIcon>
            <ListItemText>{t('records.deleteRecord')}</ListItemText>
          </MenuItem>
        )}
      </Menu>
    </>
  );
};
