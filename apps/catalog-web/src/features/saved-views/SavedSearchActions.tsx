import { Alert, Button, MenuItem, Stack, TextField } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { ExplorerSearch } from '../explorer/search';
import { SAVED_SEARCH_SELECT_MIN_WIDTH } from './constants';
import { SaveSearchDialog } from './SaveSearchDialog';
import type { SavedView } from './schemas';
import { useSavedSearchActions } from './useSavedSearchActions';

export const SavedSearchActions = ({
  search,
  savedView,
  userId,
}: {
  search: ExplorerSearch;
  savedView?: SavedView;
  userId?: string;
}) => {
  const { t } = useTranslation();
  const actions = useSavedSearchActions({ savedView, search, userId });
  const {
    busy,
    canEdit,
    error,
    isDraft,
    list,
    navigate,
    notice,
    open,
    save,
    setError,
    setNotice,
    setOpen,
  } = actions;

  return (
    <>
      <Stack direction="row" spacing={1} sx={{ my: 2, flexWrap: 'wrap' }}>
        <Button
          disabled={!search.blueprint || busy}
          onClick={() => {
            setError('');
            setOpen(true);
          }}
        >
          {t('explorer.saveSearch')}
        </Button>
        {canEdit && isDraft && (
          <Button disabled={busy} onClick={() => void actions.update()}>
            {t('explorer.saveChanges')}
          </Button>
        )}
        {isDraft && (
          <Button
            disabled={busy}
            onClick={() =>
              void navigate({
                to: '/',
                search: { savedView: search.sourceView },
              })
            }
          >
            {t('explorer.discardSearchChanges')}
          </Button>
        )}
        {canEdit && (
          <Button
            disabled={busy}
            color="error"
            onClick={() => void actions.remove()}
          >
            {t('explorer.deleteSavedSearch')}
          </Button>
        )}
        <Button
          disabled={!search.blueprint || busy}
          onClick={() => void actions.copyLink()}
        >
          {t('explorer.copyLink')}
        </Button>
        <TextField
          select
          size="small"
          label={t('explorer.savedSearches')}
          value=""
          onChange={(event) =>
            void navigate({
              to: '/',
              search: { savedView: event.target.value },
            })
          }
          sx={{ minWidth: SAVED_SEARCH_SELECT_MIN_WIDTH }}
        >
          <MenuItem value="" disabled>
            {t('explorer.savedSearches')}
          </MenuItem>
          {(list.data ?? []).map((view) => (
            <MenuItem key={view.id} value={view.id}>
              {view.name}
            </MenuItem>
          ))}
        </TextField>
      </Stack>
      {notice && (
        <Alert severity="success" onClose={() => setNotice('')}>
          {notice}
        </Alert>
      )}
      {error && (
        <Alert severity="error" onClose={() => setError('')}>
          {error}
        </Alert>
      )}
      {list.isError && <Alert severity="error">{list.error.message}</Alert>}
      <SaveSearchDialog
        error={error}
        isPending={save.isPending}
        onClose={() => setOpen(false)}
        onSave={(values) => save.mutate(values)}
        open={open}
      />
    </>
  );
};
