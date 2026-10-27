import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useInfiniteQuery } from '@tanstack/react-query';
import { Box, Typography } from '@mui/material';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { LoadMoreButton } from '../../components/LoadMoreButton';
import {
  listAuditEvents,
  type AuditEvent,
  type AuditEventFilters,
} from './api';
import { AuditEventDrawer } from './AuditEventDrawer';
import { AuditEventsTable } from './AuditEventsTable';
import { AuditFilters } from './AuditFilters';
import { auditPageSize } from './constants';
import { auditQueryKeys } from './queryKeys';
import { AuditLogIcon } from '../../components/systemIcons';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';

export const AuditLogPage = () => {
  const { t } = useTranslation();
  const systemLabel = t('audit.system');
  const workspaceLabel = t('audit.workspace');
  const [filters, setFilters] = useState<AuditEventFilters>({});
  const [draftFilters, setDraftFilters] = useState<AuditEventFilters>(filters);
  const [selected, setSelected] = useState<AuditEvent>();
  // Load more adds the next page below the events already shown.
  const events = useInfiniteQuery({
    queryKey: auditQueryKeys.events(filters),
    queryFn: ({ pageParam }) =>
      listAuditEvents({ ...filters, limit: auditPageSize, offset: pageParam }),
    initialPageParam: 0,
    getNextPageParam: (page) => {
      const next = page.offset + page.events.length;
      return page.events.length > 0 && next < page.total ? next : undefined;
    },
  });
  const loadedEvents = events.data?.pages.flatMap((page) => page.events);
  const total = events.data?.pages.at(-1)?.total;
  const updateDraft = (key: keyof AuditEventFilters, value: string) =>
    setDraftFilters((current) => ({
      ...current,
      [key]: value || undefined,
    }));
  const applyFilters = () => setFilters(draftFilters);
  return (
    <PageContainer>
      <PageHeader
        description={t('audit.description')}
        icon={AuditLogIcon}
        title={t('audit.title')}
      />
      <AuditFilters
        draft={draftFilters}
        onApply={applyFilters}
        onChange={updateDraft}
      />
      <ApiErrorAlert error={events.error} />
      <AuditEventsTable
        events={loadedEvents}
        isLoading={events.isLoading}
        onSelect={setSelected}
        systemLabel={systemLabel}
        workspaceLabel={workspaceLabel}
      />
      {loadedEvents && total !== undefined && (
        <Box sx={{ mt: 2 }}>
          <Typography color="text.secondary" variant="body2">
            {t('audit.showing', {
              from: loadedEvents.length > 0 ? 1 : 0,
              to: loadedEvents.length,
              total,
            })}
          </Typography>
          <LoadMoreButton
            disabled={!events.hasNextPage}
            isLoading={events.isFetchingNextPage}
            onLoadMore={() => void events.fetchNextPage()}
          />
        </Box>
      )}
      <AuditEventDrawer
        event={selected}
        onClose={() => setSelected(undefined)}
        systemLabel={systemLabel}
      />
    </PageContainer>
  );
};
