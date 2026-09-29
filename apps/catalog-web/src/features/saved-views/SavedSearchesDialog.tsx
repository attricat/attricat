import {
  Alert,
  Box,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  IconButton,
  List,
  ListItem,
  ListItemButton,
  ListItemText,
  Tooltip,
  Typography,
} from '@mui/material';
import { Trash2Icon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { smallIconSize } from '../../components/iconSizes';
import type { ExplorerSearch } from '../explorer/search';
import type { SavedSearchActions } from './useSavedSearchActions';

export const SavedSearchesDialog = ({
  actions,
  onClose,
  open,
  search,
}: {
  actions: SavedSearchActions;
  onClose: () => void;
  open: boolean;
  search: ExplorerSearch;
}) => {
  const { t } = useTranslation();
  const { busy, canDelete, canEdit, isDraft, list } = actions;
  const views = list.data ?? [];

  return (
    <Dialog open={open} onClose={onClose} fullWidth maxWidth="sm">
      <DialogTitle>{t('explorer.savedSearches')}</DialogTitle>
      <DialogContent dividers>
        {list.isError && <Alert severity="error">{list.error.message}</Alert>}
        {list.isSuccess && views.length === 0 && (
          <Typography color="text.secondary" variant="body2">
            {t('explorer.noSavedSearches')}
          </Typography>
        )}
        {views.length > 0 && (
          <List disablePadding>
            {views.map((view) => {
              const current = view.id === search.sourceView;
              const name = view.name ?? t('explorer.untitledSearch');
              return (
                <ListItem
                  key={view.id}
                  disablePadding
                  secondaryAction={
                    canDelete(view) && (
                      <Tooltip title={t('explorer.deleteSavedSearch')}>
                        <span>
                          <IconButton
                            aria-label={t('explorer.deleteNamedSavedSearch', {
                              name,
                            })}
                            color="error"
                            disabled={busy}
                            edge="end"
                            onClick={() => void actions.remove(view.id)}
                            size="small"
                          >
                            <Trash2Icon size={smallIconSize} />
                          </IconButton>
                        </span>
                      </Tooltip>
                    )
                  }
                >
                  <ListItemButton
                    onClick={() => {
                      void actions.openView(view.id);
                      onClose();
                    }}
                    selected={current}
                  >
                    <ListItemText
                      primary={
                        <Box
                          component="span"
                          sx={{ alignItems: 'center', display: 'flex', gap: 1 }}
                        >
                          {name}
                          {current && isDraft && (
                            <Chip
                              label={t('explorer.unsavedSearchChanges')}
                              size="small"
                            />
                          )}
                        </Box>
                      }
                      secondary={view.description || undefined}
                    />
                  </ListItemButton>
                </ListItem>
              );
            })}
          </List>
        )}
      </DialogContent>
      <DialogActions>
        {isDraft && (
          <Button
            disabled={busy}
            onClick={() => {
              void actions.discard();
              onClose();
            }}
          >
            {t('explorer.discardSearchChanges')}
          </Button>
        )}
        {canEdit && isDraft && (
          <Button
            disabled={busy}
            onClick={() => {
              void actions.update();
              onClose();
            }}
          >
            {t('explorer.saveChanges')}
          </Button>
        )}
        <Box sx={{ flexGrow: 1 }} />
        <Button onClick={onClose}>{t('common.close')}</Button>
        <Button
          disabled={!search.blueprint || busy}
          onClick={() => {
            onClose();
            actions.openSaveDialog();
          }}
          variant="contained"
        >
          {t('explorer.saveCurrentSearch')}
        </Button>
      </DialogActions>
    </Dialog>
  );
};
