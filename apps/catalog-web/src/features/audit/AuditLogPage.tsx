import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { Alert, Box, Typography } from '@mui/material';
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

export const AuditLogPage = () => {
  const { t } = useTranslation();
  const systemLabel = t('audit.system');
  const workspaceLabel = t('audit.workspace');
  const [filters, setFilters] = useState<AuditEventFilters>({
    limit: auditPageSize,
    offset: 0,
  });
  const [draftFilters, setDraftFilters] = useState<AuditEventFilters>(filters);
  const [selected, setSelected] = useState<AuditEvent>();
  const events = useQuery({
    queryKey: auditQueryKeys.events(filters),
    queryFn: () => listAuditEvents(filters),
  });
  const updateDraft = (key: keyof AuditEventFilters, value: string) =>
    setDraftFilters((current) => ({
      ...current,
      [key]: value || undefined,
    }));
  const applyFilters = () =>
    setFilters({
      ...draftFilters,
      offset: 0,
    });
  return (
    <PageContainer>
      <PageHeader
        description={t('audit.description')}
        title={t('audit.title')}
      />
      <AuditFilters
        draft={draftFilters}
        onApply={applyFilters}
        onChange={updateDraft}
      />
      {events.isError && <Alert severity="error">{events.error.message}</Alert>}
      <AuditEventsTable
        events={events.data?.events}
        isLoading={events.isLoading}
        onSelect={setSelected}
        systemLabel={systemLabel}
        workspaceLabel={workspaceLabel}
      />
      {events.data && (
        <Box sx={{ mt: 2 }}>
          <Typography color="text.secondary" variant="body2">
            {t('audit.showing', {
              from: events.data.offset + 1,
              to: events.data.offset + events.data.events.length,
              total: events.data.total,
            })}
          </Typography>
          <LoadMoreButton
            disabled={events.data.offset + auditPageSize >= events.data.total}
            isLoading={events.isFetching}
            onLoadMore={() =>
              setFilters((current) => ({
                ...current,
                offset: (current.offset ?? 0) + auditPageSize,
              }))
            }
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
