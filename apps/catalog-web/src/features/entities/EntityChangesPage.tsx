import { useInfiniteQuery, useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { EyeIcon, PencilIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Alert, Box, IconButton, Tooltip, Typography } from '@mui/material';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { LoadMoreButton } from '../../components/LoadMoreButton';
import { EntitySchemaSubheader } from './components/EntitySchemaSubheader';
import { EntityChangeEvent } from './components/EntityChangeEvent';
import { EntityToolbar } from './components/EntityToolbar';
import { getEntityChanges, getEntityForm } from './api';
import type { EntityAuditChange } from './api';
import { entityQueryKeys } from './queryKeys';

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
  const entityForm = useQuery({
    queryKey: entityQueryKeys.form(entityId),
    queryFn: () => getEntityForm(entityId),
  });
  return (
    <PageContainer maxWidth="lg">
      <PageHeader eyebrow={t('entities.entityChanges')} />
      <EntityToolbar label={t('entities.entityChanges')}>
        <Tooltip title={t('entities.backToEntity')}>
          <Link params={{ entityId }} to="/entities/$entityId">
            <IconButton aria-label={t('entities.backToEntity')}>
              <EyeIcon />
            </IconButton>
          </Link>
        </Tooltip>
        <Tooltip title={t('entities.editEntity')}>
          <Link params={{ entityId }} to="/entities/$entityId/edit">
            <IconButton aria-label={t('entities.editEntity')}>
              <PencilIcon />
            </IconButton>
          </Link>
        </Tooltip>
      </EntityToolbar>
      <EntitySchemaSubheader
        entityId={entityId}
        name={entityForm.data?.blueprint.blueprint.name}
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
