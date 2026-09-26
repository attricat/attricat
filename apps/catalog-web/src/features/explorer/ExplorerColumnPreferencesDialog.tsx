import ArrowDownwardIcon from '@mui/icons-material/ArrowDownward';
import ArrowUpwardIcon from '@mui/icons-material/ArrowUpward';
import {
  Box,
  Button,
  Checkbox,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  IconButton,
  List,
  ListItem,
  ListItemText,
  Tooltip,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { ExplorerColumnPreferences } from './columnPreferences';

const columnPreferencesListMaxHeight = 480;

type Props = {
  columns: { id: string; label: string }[];
  onChange: (preferences: ExplorerColumnPreferences) => void;
  onClear: () => void;
  onClose: () => void;
  open: boolean;
  preferences: ExplorerColumnPreferences;
};

export const ExplorerColumnPreferencesDialog = ({
  columns,
  onChange,
  onClear,
  onClose,
  open,
  preferences,
}: Props) => {
  const { t } = useTranslation();
  const labels = new Map(columns.map((column) => [column.id, column.label]));
  const move = (id: string, direction: -1 | 1) => {
    const index = preferences.order.indexOf(id);
    const target = index + direction;
    if (target < 0 || target >= preferences.order.length) return;
    const order = [...preferences.order];
    [order[index], order[target]] = [order[target], order[index]];
    onChange({ ...preferences, order });
  };

  return (
    <Dialog fullWidth maxWidth="sm" onClose={onClose} open={open}>
      <DialogTitle>{t('explorer.columnPreferences')}</DialogTitle>
      <DialogContent>
        <Box
          sx={{ maxHeight: columnPreferencesListMaxHeight, overflowY: 'auto' }}
        >
          <List aria-label={t('explorer.columns')}>
            {preferences.order.map((id, index) => (
              <ListItem
                key={id}
                secondaryAction={
                  <>
                    <Tooltip title={t('explorer.moveColumnUp')}>
                      <span>
                        <IconButton
                          aria-label={t('explorer.moveColumnUp')}
                          disabled={index === 0}
                          onClick={() => move(id, -1)}
                          size="small"
                        >
                          <ArrowUpwardIcon />
                        </IconButton>
                      </span>
                    </Tooltip>
                    <Tooltip title={t('explorer.moveColumnDown')}>
                      <span>
                        <IconButton
                          aria-label={t('explorer.moveColumnDown')}
                          disabled={index === preferences.order.length - 1}
                          onClick={() => move(id, 1)}
                          size="small"
                        >
                          <ArrowDownwardIcon />
                        </IconButton>
                      </span>
                    </Tooltip>
                  </>
                }
              >
                <Checkbox
                  checked={!preferences.hidden.includes(id)}
                  edge="start"
                  slotProps={{
                    input: {
                      'aria-label': t('explorer.showColumn', {
                        column: labels.get(id),
                      }),
                    },
                  }}
                  onChange={() =>
                    onChange({
                      ...preferences,
                      hidden: preferences.hidden.includes(id)
                        ? preferences.hidden.filter((hidden) => hidden !== id)
                        : [...preferences.hidden, id],
                    })
                  }
                />
                <ListItemText primary={labels.get(id) ?? id} />
              </ListItem>
            ))}
          </List>
        </Box>
      </DialogContent>
      <DialogActions>
        <Button color="inherit" onClick={onClear}>
          {t('explorer.clearColumnPreferences')}
        </Button>
        <Button onClick={onClose} variant="contained">
          {t('explorer.done')}
        </Button>
      </DialogActions>
    </Dialog>
  );
};
