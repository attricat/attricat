import { useInfiniteQuery, useQuery } from '@tanstack/react-query';
import { ArrowLeftIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Alert, Box, Typography } from '@mui/material';
import { PageContainer } from '../../components/PageContainer';
import { RouterButton } from '../../components/RouterLink';
import { PageHeader } from '../../components/PageHeader';
import { LoadMoreButton } from '../../components/LoadMoreButton';
import { RecordSchemaSubheader } from './components/RecordSchemaSubheader';
import { RecordChangeEvent } from './components/RecordChangeEvent';
import { RecordToolbar } from './components/RecordToolbar';
import { getRecordChanges } from './api';
import { recordFormOptions } from './queryOptions';
import type { RecordAuditChange } from './api';
import { recordQueryKeys } from './queryKeys';
import { lexiconText } from '../lexicon/lexicon';

type EventChanges = [RecordAuditChange, ...RecordAuditChange[]];

const groupChangesByEvent = (changes: RecordAuditChange[]) => [
  ...changes
    .reduce((groups, change) => {
      const group = groups.get(change.audit_event_id);
      if (group) group.push(change);
      else groups.set(change.audit_event_id, [change]);
      return groups;
    }, new Map<string, EventChanges>())
    .entries(),
];

export const RecordChangesPage = ({ recordId }: { recordId: string }) => {
  const { t } = useTranslation();
  const changes = useInfiniteQuery({
    queryKey: recordQueryKeys.changes(recordId),
    queryFn: ({ pageParam }) => getRecordChanges(recordId, pageParam),
    initialPageParam: 0,
    getNextPageParam: (page) => page.next_offset,
  });
  const changeItems = changes.data?.pages.flatMap((page) => page.items) ?? [];
  const recordForm = useQuery(recordFormOptions(recordId));
  return (
    <PageContainer maxWidth="lg">
      <PageHeader eyebrow={t('records.recordChanges')} />
      <RecordToolbar label={t('records.recordChanges')}>
        <RouterButton
          color="inherit"
          params={{ recordId }}
          size="small"
          startIcon={<ArrowLeftIcon />}
          to="/records/$recordId"
        >
          {t('records.backToRecord')}
        </RouterButton>
      </RecordToolbar>
      <RecordSchemaSubheader
        name={
          recordForm.data &&
          lexiconText(recordForm.data.blueprint.blueprint.name)
        }
      />
      {changes.isPending && (
        <Typography sx={{ py: 3 }}>{t('records.loadingChanges')}</Typography>
      )}
      {changes.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {changes.error.message}
        </Alert>
      )}
      {changes.data && (
        <Box sx={{ mt: 3 }}>
          {groupChangesByEvent(changeItems).map(([eventId, eventChanges]) => (
            <RecordChangeEvent changes={eventChanges} key={eventId} />
          ))}
          {changeItems.length === 0 && (
            <Typography>{t('records.noRecordedChanges')}</Typography>
          )}
          {changes.isFetchNextPageError && (
            <Alert severity="error" sx={{ mb: 2 }}>
              {changes.error.message}
            </Alert>
          )}
          {changes.hasNextPage && (
            <LoadMoreButton
              isLoading={changes.isFetchingNextPage}
              onLoadMore={() => void changes.fetchNextPage()}
            />
          )}
        </Box>
      )}
    </PageContainer>
  );
};
