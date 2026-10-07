import { useInfiniteQuery, useQuery } from '@tanstack/react-query';
import { ArrowLeftIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Alert, Box, Typography } from '@mui/material';
import { PageContainer } from '../../components/PageContainer';
import { RouterButton } from '../../components/RouterLink';
import { PageHeader } from '../../components/PageHeader';
import { LoadMoreButton } from '../../components/LoadMoreButton';
import { EntitySchemaSubheader } from './components/EntitySchemaSubheader';
import { EntityChangeEvent } from './components/EntityChangeEvent';
import { EntityToolbar } from './components/EntityToolbar';
import { getEntityChanges } from './api';
import { entityFormOptions } from './queryOptions';
import type { EntityAuditChange } from './api';
import { entityQueryKeys } from './queryKeys';
import { lexiconText } from '../lexicon/lexicon';

type EventChanges = [EntityAuditChange, ...EntityAuditChange[]];

const groupChangesByEvent = (changes: EntityAuditChange[]) => [
  ...changes
    .reduce((groups, change) => {
      const group = groups.get(change.audit_event_id);
      if (group) group.push(change);
      else groups.set(change.audit_event_id, [change]);
      return groups;
    }, new Map<string, EventChanges>())
    .entries(),
];

export const EntityChangesPage = ({ entityId }: { entityId: string }) => {
  const { t } = useTranslation();
  const changes = useInfiniteQuery({
    queryKey: entityQueryKeys.changes(entityId),
    queryFn: ({ pageParam }) => getEntityChanges(entityId, pageParam),
    initialPageParam: 0,
    getNextPageParam: (page) => page.next_offset,
  });
  const changeItems = changes.data?.pages.flatMap((page) => page.items) ?? [];
  const entityForm = useQuery(entityFormOptions(entityId));
  return (
    <PageContainer maxWidth="lg">
      <PageHeader eyebrow={t('entities.entityChanges')} />
      <EntityToolbar label={t('entities.entityChanges')}>
        <RouterButton
          color="inherit"
          params={{ entityId }}
          size="small"
          startIcon={<ArrowLeftIcon />}
          to="/entities/$entityId"
        >
          {t('entities.backToEntity')}
        </RouterButton>
      </EntityToolbar>
      <EntitySchemaSubheader
        entityId={entityId}
        name={
          entityForm.data &&
          lexiconText(entityForm.data.blueprint.blueprint.name)
        }
      />
      {changes.isPending && (
        <Typography sx={{ py: 3 }}>{t('entities.loadingChanges')}</Typography>
      )}
      {changes.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {changes.error.message}
        </Alert>
      )}
      {changes.data && (
        <Box sx={{ mt: 3 }}>
          {groupChangesByEvent(changeItems).map(([eventId, eventChanges]) => (
            <EntityChangeEvent changes={eventChanges} key={eventId} />
          ))}
          {changeItems.length === 0 && (
            <Typography>{t('entities.noRecordedChanges')}</Typography>
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
