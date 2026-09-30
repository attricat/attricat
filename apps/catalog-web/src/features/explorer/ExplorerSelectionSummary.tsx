import {
  Button,
  IconButton,
  List,
  ListItem,
  ListItemText,
  Popover,
  Tooltip,
} from '@mui/material';
import { ChevronDownIcon, XIcon } from 'lucide-react';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../components/iconSizes';
import type { EntityItem } from '../entities/api';
import { displayLabel } from '../entities/entityDisplay';
import {
  selectedEntityListMaxHeight,
  selectedEntityListWidth,
} from './constants';

type Props = {
  selectedItems: EntityItem[];
  onClear: () => void;
  onRemove: (entityId: string) => void;
};

export const ExplorerSelectionSummary = ({
  selectedItems,
  onClear,
  onRemove,
}: Props) => {
  const { t } = useTranslation();
  const listId = useId();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const open = Boolean(anchor) && selectedItems.length > 0;

  return (
    <>
      <Button
        aria-controls={open ? listId : undefined}
        aria-expanded={open}
        aria-haspopup="dialog"
        color="inherit"
        disabled={selectedItems.length === 0}
        endIcon={<ChevronDownIcon />}
        onClick={(event) => setAnchor(event.currentTarget)}
        size="small"
      >
        {t('explorer.selectedCount', { count: selectedItems.length })}
      </Button>
      {selectedItems.length > 0 && (
        <Tooltip title={t('explorer.clearSelection')}>
          <IconButton
            aria-label={t('explorer.clearSelection')}
            onClick={onClear}
            size="small"
          >
            <XIcon size={compactIconSize} />
          </IconButton>
        </Tooltip>
      )}
      <Popover
        anchorEl={anchor}
        anchorOrigin={{ horizontal: 'left', vertical: 'bottom' }}
        id={listId}
        onClose={() => setAnchor(null)}
        open={open}
        slotProps={{
          paper: {
            'aria-label': t('explorer.selectedEntities'),
            role: 'dialog',
            sx: {
              maxHeight: selectedEntityListMaxHeight,
              width: selectedEntityListWidth,
            },
          },
        }}
        transformOrigin={{ horizontal: 'left', vertical: 'top' }}
      >
        <List dense>
          {selectedItems.map((entity) => {
            const label = displayLabel(entity.display, entity.id);
            return (
              <ListItem
                key={entity.id}
                secondaryAction={
                  <IconButton
                    aria-label={t('explorer.removeFromSelection', {
                      entity: label,
                    })}
                    edge="end"
                    onClick={() => onRemove(entity.id)}
                    size="small"
                  >
                    <XIcon size={compactIconSize} />
                  </IconButton>
                }
              >
                <ListItemText
                  primary={label}
                  slotProps={{ primary: { noWrap: true } }}
                />
              </ListItem>
            );
          })}
        </List>
      </Popover>
    </>
  );
};
