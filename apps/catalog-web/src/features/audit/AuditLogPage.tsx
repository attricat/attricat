import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import {
  Alert,
  Box,
  Button,
  Chip,
  Drawer,
  MenuItem,
  Paper,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  TextField,
  Typography,
} from '@mui/material';
import { PageContainer } from '../../components/PageContainer';
import { RouterButton } from '../../components/RouterLink';
import { PageHeader } from '../../components/PageHeader';
import { LoadMoreButton } from '../../components/LoadMoreButton';
import {
  listAuditEvents,
  type AuditEvent,
  type AuditEventFilters,
} from './api';
import { auditQueryKeys } from './queryKeys';

const pageSize = 50;
const formatDate = (value: string, locale: string) =>
  new Intl.DateTimeFormat(locale, {
    dateStyle: 'medium',
    timeStyle: 'medium',
  }).format(new Date(value));
const actor = (event: AuditEvent, systemLabel: string) =>
  event.actor_display_name ??
  event.actor_email ??
  event.actor_user_id ??
  systemLabel;
const target = (event: AuditEvent, workspaceLabel: string) =>
  Object.entries(event.target)
    .map(([key, value]) => `${key}: ${String(value)}`)
    .join(', ') || workspaceLabel;

export const AuditLogPage = () => {
  const { i18n, t } = useTranslation();
  const locale = i18n.resolvedLanguage ?? i18n.language;
  const systemLabel = t('audit.system');
  const workspaceLabel = t('audit.workspace');
  const [filters, setFilters] = useState<AuditEventFilters>({
    limit: pageSize,
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
      <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 2, mb: 3 }}>
        <TextField
          label={t('audit.from')}
          onChange={(event) =>
            updateDraft(
              'occurred_after',
              event.target.value
                ? new Date(event.target.value).toISOString()
                : '',
            )
          }
          size="small"
          type="datetime-local"
        />
        <TextField
          label={t('audit.to')}
          onChange={(event) =>
            updateDraft(
              'occurred_before',
              event.target.value
                ? new Date(event.target.value).toISOString()
                : '',
            )
          }
          size="small"
          type="datetime-local"
        />
        <TextField
          label={t('audit.actionCategory')}
          onChange={(event) =>
            updateDraft('action_category', event.target.value)
          }
          placeholder={t('audit.actionCategoryPlaceholder')}
          size="small"
        />
        <TextField
          label={t('audit.actorId')}
          onChange={(event) => updateDraft('actor_user_id', event.target.value)}
          size="small"
        />
        <TextField
          label={t('audit.targetType')}
          onChange={(event) => updateDraft('target_type', event.target.value)}
          size="small"
        />
        <TextField
          label={t('audit.executor')}
          onChange={(event) => updateDraft('executor_type', event.target.value)}
          select
          size="small"
          value={draftFilters.executor_type ?? ''}
        >
          <MenuItem value="">{t('audit.all')}</MenuItem>
          <MenuItem value="human">{t('audit.human')}</MenuItem>
          <MenuItem value="agent">{t('audit.agent')}</MenuItem>
        </TextField>
        <TextField
          label={t('audit.agentRunId')}
          onChange={(event) => updateDraft('agent_run_id', event.target.value)}
          size="small"
        />
        <TextField
          label={t('audit.toolCallId')}
          onChange={(event) =>
            updateDraft('agent_tool_call_id', event.target.value)
          }
          size="small"
        />
        <Button onClick={applyFilters} variant="contained">
          {t('audit.applyFilters')}
        </Button>
      </Box>
      {events.isError && <Alert severity="error">{events.error.message}</Alert>}
      <Paper variant="outlined">
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell>{t('audit.timestamp')}</TableCell>
              <TableCell>{t('audit.actor')}</TableCell>
              <TableCell>{t('audit.source')}</TableCell>
              <TableCell>{t('audit.action')}</TableCell>
              <TableCell>{t('audit.target')}</TableCell>
              <TableCell>{t('audit.outcome')}</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {events.data?.events.map((event) => (
              <TableRow hover key={event.id}>
                <TableCell>
                  <Button
                    aria-label={t('audit.viewEvent', {
                      action: event.action,
                      actor: actor(event, systemLabel),
                    })}
                    onClick={() => setSelected(event)}
                    size="small"
                    variant="text"
                  >
                    {formatDate(event.occurred_at, locale)}
                  </Button>
                </TableCell>
                <TableCell>{actor(event, systemLabel)}</TableCell>
                <TableCell>
                  <Chip
                    color={
                      event.executor_type === 'agent' ? 'secondary' : 'default'
                    }
                    label={event.executor_type}
                    size="small"
                  />
                </TableCell>
                <TableCell>{event.action}</TableCell>
                <TableCell>{target(event, workspaceLabel)}</TableCell>
                <TableCell>
                  <Chip
                    color={event.outcome === 'success' ? 'success' : 'default'}
                    label={event.outcome}
                    size="small"
                  />
                </TableCell>
              </TableRow>
            ))}
            {!events.isLoading && events.data?.events.length === 0 && (
              <TableRow>
                <TableCell colSpan={6}>{t('audit.noMatches')}</TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </Paper>
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
            disabled={events.data.offset + pageSize >= events.data.total}
            isLoading={events.isFetching}
            onLoadMore={() =>
              setFilters((current) => ({
                ...current,
                offset: current.offset! + pageSize,
              }))
            }
          />
        </Box>
      )}
      <EventDrawer
        event={selected}
        locale={locale}
        onClose={() => setSelected(undefined)}
        systemLabel={systemLabel}
      />
    </PageContainer>
  );
};

const EventDrawer = ({
  event,
  locale,
  onClose,
  systemLabel,
}: {
  event?: AuditEvent;
  locale: string;
  onClose: () => void;
  systemLabel: string;
}) => {
  const { t } = useTranslation();
  return (
    <Drawer
      anchor="right"
      aria-labelledby={event ? `audit-event-${event.id}` : undefined}
      onClose={onClose}
      open={Boolean(event)}
    >
      <Box sx={{ p: 3, width: { xs: '100vw', sm: 480 } }}>
        <Typography
          component="h2"
          id={event ? `audit-event-${event.id}` : undefined}
          variant="h6"
        >
          {t('audit.event')}
        </Typography>
        {event && (
          <Box sx={{ display: 'grid', gap: 2, mt: 2 }}>
            <Typography>
              {t('audit.eventBy', {
                action: event.action,
                actor: actor(event, systemLabel),
                date: formatDate(event.occurred_at, locale),
              })}
            </Typography>
            {event.agent_conversation_id && (
              <RouterButton
                params={{ conversationId: event.agent_conversation_id }}
                to="/agents/$conversationId"
              >
                {t('audit.openConversation')}
              </RouterButton>
            )}
            <Detail
              label={t('audit.authorizationScope')}
              value={event.authorization_scope}
            />
            <Detail label={t('audit.target')} value={event.target} />
            <Detail label={t('audit.safeMetadata')} value={event.metadata} />
            <Detail label={t('audit.requestId')} value={event.request_id} />
            <Detail
              label={t('audit.correlationId')}
              value={event.correlation_id}
            />
            {event.approval_decision && (
              <Detail
                label={t('audit.agentApproval')}
                value={`${event.approval_decision} by ${event.approved_by_display_name ?? event.approved_by_email ?? event.approved_by_user_id}`}
              />
            )}
            {event.agent_tool_call_id && (
              <Detail
                label={t('audit.agentToolCall')}
                value={`${event.agent_tool_name}: ${event.agent_tool_call_id}`}
              />
            )}
          </Box>
        )}
      </Box>
    </Drawer>
  );
};
const Detail = ({ label, value }: { label: string; value: unknown }) => (
  <Box>
    <Typography color="text.secondary" variant="caption">
      {label}
    </Typography>
    <Typography
      component="pre"
      sx={{ m: 0, overflowWrap: 'anywhere', whiteSpace: 'pre-wrap' }}
      variant="body2"
    >
      {typeof value === 'string' ? value : JSON.stringify(value, null, 2)}
    </Typography>
  </Box>
);
