import { useState } from 'react';
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
import { PageHeader } from '../../components/PageHeader';
import { LoadMoreButton } from '../../components/LoadMoreButton';
import {
  listAuditEvents,
  type AuditEvent,
  type AuditEventFilters,
} from './api';
import { auditQueryKeys } from './query-keys';

const pageSize = 50;
const formatDate = (value: string) =>
  new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'medium',
  }).format(new Date(value));
const actor = (event: AuditEvent) =>
  event.actor_display_name ??
  event.actor_email ??
  event.actor_user_id ??
  'System';
const target = (event: AuditEvent) =>
  Object.entries(event.target)
    .map(([key, value]) => `${key}: ${String(value)}`)
    .join(', ') || 'Workspace';

export const AuditLogPage = () => {
  const [filters, setFilters] = useState<AuditEventFilters>({
    limit: pageSize,
    offset: 0,
  });
  const [selected, setSelected] = useState<AuditEvent>();
  const events = useQuery({
    queryKey: auditQueryKeys.events(filters),
    queryFn: () => listAuditEvents(filters),
  });
  const update = (key: keyof AuditEventFilters, value: string) =>
    setFilters((current) => ({
      ...current,
      [key]: value || undefined,
      offset: 0,
    }));
  return (
    <PageContainer>
      <PageHeader
        description="Successful workspace mutations, including agent activity."
        title="Activity / Audit log"
      />
      <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 2, mb: 3 }}>
        <TextField
          label="From"
          onChange={(event) =>
            update(
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
          label="To"
          onChange={(event) =>
            update(
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
          label="Action category"
          onChange={(event) => update('action_category', event.target.value)}
          placeholder="catalog"
          size="small"
        />
        <TextField
          label="Actor ID"
          onChange={(event) => update('actor_user_id', event.target.value)}
          size="small"
        />
        <TextField
          label="Target type"
          onChange={(event) => update('target_type', event.target.value)}
          size="small"
        />
        <TextField
          label="Executor"
          onChange={(event) => update('executor_type', event.target.value)}
          select
          size="small"
          value={filters.executor_type ?? ''}
        >
          <MenuItem value="">All</MenuItem>
          <MenuItem value="human">Human</MenuItem>
          <MenuItem value="agent">Agent</MenuItem>
        </TextField>
        <TextField
          label="Agent run ID"
          onChange={(event) => update('agent_run_id', event.target.value)}
          size="small"
        />
        <TextField
          label="Tool-call ID"
          onChange={(event) => update('agent_tool_call_id', event.target.value)}
          size="small"
        />
      </Box>
      {events.isError && <Alert severity="error">{events.error.message}</Alert>}
      <Paper variant="outlined">
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell>Timestamp</TableCell>
              <TableCell>Actor</TableCell>
              <TableCell>Source</TableCell>
              <TableCell>Action</TableCell>
              <TableCell>Target</TableCell>
              <TableCell>Outcome</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {events.data?.events.map((event) => (
              <TableRow
                hover
                key={event.id}
                onClick={() => setSelected(event)}
                sx={{ cursor: 'pointer' }}
              >
                <TableCell>{formatDate(event.occurred_at)}</TableCell>
                <TableCell>{actor(event)}</TableCell>
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
                <TableCell>{target(event)}</TableCell>
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
                <TableCell colSpan={6}>
                  No activity matches these filters.
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </Paper>
      {events.data && (
        <Box sx={{ mt: 2 }}>
          <Typography color="text.secondary" variant="body2">
            Showing {events.data.offset + 1}-
            {events.data.offset + events.data.events.length} of{' '}
            {events.data.total}
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
      <EventDrawer event={selected} onClose={() => setSelected(undefined)} />
    </PageContainer>
  );
};

const EventDrawer = ({
  event,
  onClose,
}: {
  event?: AuditEvent;
  onClose: () => void;
}) => (
  <Drawer anchor="right" onClose={onClose} open={Boolean(event)}>
    <Box sx={{ p: 3, width: { xs: '100vw', sm: 480 } }}>
      <Typography variant="h6">Audit event</Typography>
      {event && (
        <Box sx={{ display: 'grid', gap: 2, mt: 2 }}>
          <Typography>
            <b>{event.action}</b> by {actor(event)} at{' '}
            {formatDate(event.occurred_at)}
          </Typography>
          {event.agent_conversation_id && (
            <Button href={`/agents/${event.agent_conversation_id}`}>
              Open agent conversation
            </Button>
          )}
          <Detail
            label="Authorization scope"
            value={event.authorization_scope}
          />
          <Detail label="Target" value={event.target} />
          <Detail label="Safe metadata" value={event.metadata} />
          <Detail label="Request ID" value={event.request_id} />
          <Detail label="Correlation ID" value={event.correlation_id} />
          {event.approval_decision && (
            <Detail
              label="Agent approval"
              value={`${event.approval_decision} by ${event.approved_by_display_name ?? event.approved_by_email ?? event.approved_by_user_id}`}
            />
          )}
          {event.agent_tool_call_id && (
            <Detail
              label="Agent tool call"
              value={`${event.agent_tool_name}: ${event.agent_tool_call_id}`}
            />
          )}
        </Box>
      )}
    </Box>
  </Drawer>
);
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
