import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { Alert, Box, Paper, Typography } from '@mui/material';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { getEntityChanges } from './api';
import type { EntityAuditChange } from './api';
import { entityQueryKeys } from './query-keys';

const groupChangesByEvent = (changes: EntityAuditChange[]) =>
  changes.reduce<Record<string, EntityAuditChange[]>>((groups, change) => {
    (groups[change.audit_event_id] ??= []).push(change);
    return groups;
  }, {});

export const EntityChangesPage = ({ entityId }: { entityId: string }) => {
  const { t } = useTranslation();
  const changes = useQuery({
    queryKey: entityQueryKeys.changes(entityId),
    queryFn: () => getEntityChanges(entityId),
  });
  return (
    <PageContainer maxWidth="lg">
      <PageHeader eyebrow={t('entities.entityChanges')} />
      <Link params={{ entityId }} to="/entities/$entityId">
        {t('entities.backToEntity')}
      </Link>
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
          {Object.entries(groupChangesByEvent(changes.data)).map(
            ([eventId, eventChanges]) => {
              const event = eventChanges[0];
              const actor =
                event.actor_display_name ??
                event.actor_email ??
                (event.executor_type === 'agent'
                  ? t('entities.agent')
                  : t('entities.unknownActor'));
              return (
                <Paper component="section" key={eventId} sx={{ mb: 2, p: 2 }}>
                  <Typography sx={{ fontWeight: 'bold' }}>
                    {actor} · {new Date(event.occurred_at).toLocaleString()}
                  </Typography>
                  <Typography color="text.secondary" variant="body2">
                    {event.approval_decision
                      ? event.approved_by_display_name
                        ? t('entities.approvalBy', {
                            decision: event.approval_decision,
                            actor: event.approved_by_display_name,
                          })
                        : t('entities.approval', {
                            decision: event.approval_decision,
                          })
                      : t('entities.noApproval')}
                  </Typography>
                  {eventChanges.map((change) => (
                    <Box
                      key={`${change.attribute_id}-${change.context_id}-${change.change_kind}`}
                      sx={{ mt: 1 }}
                    >
                      <Typography variant="body2">
                        <strong>{change.attribute_code}</strong>
                        {change.context_code ? ` (${change.context_code})` : ''}
                        : {change.change_kind.replaceAll('_', ' ')}
                      </Typography>
                      <Typography
                        component="pre"
                        sx={{
                          fontFamily: 'monospace',
                          m: 0,
                          whiteSpace: 'pre-wrap',
                        }}
                        variant="body2"
                      >
                        {JSON.stringify(change.before_value)} →{' '}
                        {JSON.stringify(change.after_value)}
                      </Typography>
                    </Box>
                  ))}
                </Paper>
              );
            },
          )}
          {changes.data.length === 0 && (
            <Typography>{t('entities.noRecordedChanges')}</Typography>
          )}
        </Box>
      )}
    </PageContainer>
  );
};
