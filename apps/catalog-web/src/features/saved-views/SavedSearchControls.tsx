import { IconButton, Tooltip } from '@mui/material';
import { Share2Icon } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { smallIconSize } from '../../components/iconSizes';
import { SavedSearchIcon } from '../../components/systemIcons';
import type { ExplorerSearch } from '../explorer/search';
import { SavedSearchesDialog } from './SavedSearchesDialog';
import { SaveSearchDialog } from './SaveSearchDialog';
import type { SavedSearchActions } from './useSavedSearchActions';

type Props = {
  actions: SavedSearchActions;
  search: ExplorerSearch;
};

export const SavedSearchesButton = ({ actions, search }: Props) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const { save, sourceView } = actions;
  const label = sourceView
    ? t('explorer.savedSearchActive', {
        name: sourceView.name ?? t('explorer.untitledSearch'),
      })
    : t('explorer.savedSearches');

  return (
    <>
      <Tooltip title={label}>
        <IconButton
          aria-label={label}
          aria-pressed={Boolean(search.sourceView)}
          color={search.sourceView ? 'primary' : 'default'}
          onClick={() => setOpen(true)}
          sx={
            search.sourceView
              ? {
                  bgcolor: 'action.selected',
                  '&:hover': { bgcolor: 'action.focus' },
                }
              : undefined
          }
        >
          <SavedSearchIcon size={smallIconSize} />
        </IconButton>
      </Tooltip>
      <SavedSearchesDialog
        actions={actions}
        onClose={() => setOpen(false)}
        open={open}
        search={search}
      />
      <SaveSearchDialog
        error={actions.error}
        isPending={save.isPending}
        onClose={actions.closeSaveDialog}
        onSave={(values) => save.mutate(values)}
        open={actions.saveOpen}
      />
    </>
  );
};

export const ShareSearchButton = ({ actions, search }: Props) => {
  const { t } = useTranslation();
  const disabled = !search.blueprint || actions.busy;

  return (
    <Tooltip title={t('explorer.shareSearch')}>
      <span>
        <IconButton
          aria-label={t('explorer.shareSearch')}
          disabled={disabled}
          onClick={() => void actions.copyLink()}
        >
          <Share2Icon size={smallIconSize} />
        </IconButton>
      </span>
    </Tooltip>
  );
};
