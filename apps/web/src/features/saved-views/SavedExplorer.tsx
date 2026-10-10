import { useQuery } from '@tanstack/react-query';
import { Alert, CircularProgress } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { Explorer } from '../explorer/Explorer';
import type { ExplorerSearch } from '../explorer/search';
import { getSavedView } from './api';
import { savedViewQueryKeys } from './queryKeys';

export const SavedExplorer = ({
  panelRecordId,
  search,
}: {
  panelRecordId?: string;
  search: ExplorerSearch;
}) => {
  const { t } = useTranslation();
  const id = search.savedView ?? search.viewState ?? '';
  const link = Boolean(search.viewState && !search.savedView);
  const view = useQuery({
    queryKey: savedViewQueryKeys.detail(id, link),
    queryFn: ({ signal }) => getSavedView(id, link, signal),
    enabled: Boolean(id),
  });
  if (!id) return <Explorer panelRecordId={panelRecordId} search={search} />;
  if (view.isPending)
    return <CircularProgress aria-label={t('explorer.loadingSavedSearch')} />;
  if (view.isError) return <Alert severity="error">{view.error.message}</Alert>;
  return (
    <Explorer
      panelRecordId={panelRecordId}
      search={{ ...view.data.state, sourceView: link ? undefined : id }}
      savedView={link ? undefined : view.data}
    />
  );
};
