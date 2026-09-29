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
  InputAdornment,
  LinearProgress,
  List,
  ListItem,
  ListItemButton,
  ListItemText,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { SearchIcon, Trash2Icon } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { smallIconSize } from '../../components/iconSizes';
import type { ExplorerSearch } from '../explorer/search';
import { listSavedViews } from './api';
import {
  SAVED_VIEW_SEARCH_DEBOUNCE_MS,
  SAVED_VIEW_SEARCH_MAX_LENGTH,
} from './constants';
import { savedViewQueryKeys } from './queryKeys';
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
  const { busy, canDelete, canEdit, isDraft } = actions;
  const [filter, setFilter] = useState('');
  const [searchText, setSearchText] = useState('');
  useEffect(() => {
    const timeout = window.setTimeout(
      () => setSearchText(filter.trim()),
      SAVED_VIEW_SEARCH_DEBOUNCE_MS,
    );
    return () => window.clearTimeout(timeout);
  }, [filter]);
  const list = useQuery({
    queryKey: savedViewQueryKeys.list(searchText),
    queryFn: ({ signal }) => listSavedViews(searchText, signal),
    enabled: open,
    placeholderData: keepPreviousData,
  });
  const views = list.data ?? [];

  return (
    <Dialog open={open} onClose={onClose} fullWidth maxWidth="sm">
      <DialogTitle>{t('explorer.savedSearches')}</DialogTitle>
      <DialogContent dividers>
        <TextField
          fullWidth
          label={t('explorer.filterSavedSearches')}
          onChange={(event) => setFilter(event.target.value)}
          size="small"
          slotProps={{
            htmlInput: { maxLength: SAVED_VIEW_SEARCH_MAX_LENGTH },
            input: {
              startAdornment: (
                <InputAdornment position="start">
                  <SearchIcon size={smallIconSize} />
                </InputAdornment>
              ),
            },
          }}
          sx={{ mb: 1.5, mt: 0.5 }}
          type="search"
          value={filter}
        />
        {list.isFetching && list.isPlaceholderData && (
          <LinearProgress sx={{ mb: 1 }} />
        )}
        {list.isError && <Alert severity="error">{list.error.message}</Alert>}
        {list.isSuccess && views.length === 0 && (
          <Typography color="text.secondary" variant="body2">
            {searchText
              ? t('explorer.noMatchingSavedSearches')
              : t('explorer.noSavedSearches')}
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
        {search.sourceView && (
          <Button
            disabled={busy}
            onClick={() => {
              void actions.clear();
              onClose();
            }}
          >
            {t('explorer.clearSavedSearch')}
          </Button>
        )}
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
