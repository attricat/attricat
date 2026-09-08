import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import EditOutlinedIcon from '@mui/icons-material/EditOutlined';
import VisibilityOutlinedIcon from '@mui/icons-material/VisibilityOutlined';
import { useTranslation } from 'react-i18next';
import {
  Alert,
  Box,
  IconButton,
  Paper,
  Tooltip,
  Typography,
} from '@mui/material';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { EntitySchemaSubheader } from './components/EntitySchemaSubheader';
import { EntityToolbar } from './components/EntityToolbar';
import { getEntityChanges, getEntityForm } from './api';
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
              <VisibilityOutlinedIcon />
            </IconButton>
          </Link>
        </Tooltip>
        <Tooltip title={t('entities.editEntity')}>
          <Link params={{ entityId }} to="/entities/$entityId/edit">
            <IconButton aria-label={t('entities.editEntity')}>
              <EditOutlinedIcon />
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
