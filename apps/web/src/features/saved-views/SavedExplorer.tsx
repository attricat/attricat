import { useQuery } from '@tanstack/react-query';
import { CircularProgress } from '@mui/material';
import { memo, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Explorer } from '../explorer/Explorer';
import type { ExplorerSearch } from '../explorer/search';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { savedViewOptions } from './queryOptions';

// Memoized so that changes of the route that leave the search alone, such as
// opening a record in the panel, do not re-render the results.
export const SavedExplorer = memo(function SavedExplorer({
  search,
}: {
  search: ExplorerSearch;
}) {
  const { t } = useTranslation();
  const id = search.savedView ?? search.viewState ?? '';
  const link = Boolean(search.viewState && !search.savedView);
  const view = useQuery({
    ...savedViewOptions(id, link),
    enabled: Boolean(id),
  });
  const savedSearch = useMemo(
    () =>
      view.data && { ...view.data.state, sourceView: link ? undefined : id },
    [id, link, view.data],
  );
  if (!id) return <Explorer search={search} />;
  if (view.isPending)
    return <CircularProgress aria-label={t('explorer.loadingSavedSearch')} />;
  if (view.isError) return <ApiErrorAlert error={view.error} />;
  return (
    <Explorer
      search={savedSearch ?? view.data.state}
      savedView={link ? undefined : view.data}
    />
  );
});
