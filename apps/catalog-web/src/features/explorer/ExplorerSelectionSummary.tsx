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
import type { RecordItem } from '../records/api';
import { displayLabel } from '../records/recordDisplay';
import {
  selectedRecordListMaxHeight,
  selectedRecordListWidth,
} from './constants';

type Props = {
  selectedItems: RecordItem[];
  onClear: () => void;
  onRemove: (recordId: string) => void;
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
            'aria-label': t('explorer.selectedRecords'),
            role: 'dialog',
            sx: {
              maxHeight: selectedRecordListMaxHeight,
              width: selectedRecordListWidth,
            },
          },
        }}
        transformOrigin={{ horizontal: 'left', vertical: 'top' }}
      >
        <List dense>
          {selectedItems.map((record) => {
            const label = displayLabel(record.display, record.id);
            return (
              <ListItem
                key={record.id}
                secondaryAction={
                  <IconButton
                    aria-label={t('explorer.removeFromSelection', {
                      record: label,
                    })}
                    edge="end"
                    onClick={() => onRemove(record.id)}
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
