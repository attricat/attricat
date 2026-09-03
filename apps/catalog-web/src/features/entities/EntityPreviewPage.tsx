import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import CheckCircleOutlinedIcon from '@mui/icons-material/CheckCircleOutlined';
import WarningAmberOutlinedIcon from '@mui/icons-material/WarningAmberOutlined';
import {
  Alert,
  Box,
  MenuItem,
  Paper,
  Tab,
  Tabs,
  TextField,
  Typography,
} from '@mui/material';
import { createElement, useState } from 'react';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import {
  getBlueprintRevision,
  getCurrentBlueprint,
  getEntityChanges,
  getResolvedEntityPreview,
  listContexts,
} from './api';
import { entityQueryKeys } from './query-keys';
import { EntityView } from '../views/components/EntityView';
import {
  entityHeadingComponentId,
  findEntityHeading,
} from '../views/components/blocks/EntityHeadingDefinition';
import { resolveHeadingRenderer } from '../views/components/registry';
import type { EntityAuditChange } from './api';

const groupChangesByEvent = (changes: EntityAuditChange[]) =>
  changes.reduce<Record<string, EntityAuditChange[]>>((groups, change) => {
    (groups[change.audit_event_id] ??= []).push(change);
    return groups;
  }, {});

export const EntityPreviewPage = ({ entityId }: { entityId: string }) => {
  const [selectedContext, setSelectedContext] = useState('default');
  const [tab, setTab] = useState<'preview' | 'changes'>('preview');
  const contexts = useQuery({
    queryKey: entityQueryKeys.contexts(),
    queryFn: listContexts,
  });
  const selectedContextId = contexts.data?.find(
    (context) => context.code === selectedContext,
  )?.id;
  const resolved = useQuery({
    queryKey: selectedContextId
      ? entityQueryKeys.resolvedPreview(entityId, selectedContextId)
      : ['entity-resolved-preview'],
    queryFn: () => getResolvedEntityPreview(entityId, selectedContextId!),
    enabled: Boolean(selectedContextId),
  });
  const blueprint = useQuery({
    queryKey: resolved.data
      ? entityQueryKeys.blueprintRevision(
          resolved.data.entity.blueprint_id!,
          resolved.data.entity.blueprint_version!,
        )
      : ['blueprint-revision'],
    queryFn: () =>
      getBlueprintRevision(
        resolved.data!.entity.blueprint_id!,
        resolved.data!.entity.blueprint_version!,
      ),
    enabled: Boolean(
      resolved.data?.entity.blueprint_id &&
      resolved.data.entity.blueprint_version,
    ),
  });
  const currentBlueprint = useQuery({
    queryKey: entityQueryKeys.currentBlueprint(
      resolved.data?.entity.blueprint_id ?? '',
    ),
    queryFn: () => getCurrentBlueprint(resolved.data!.entity.blueprint_id!),
    enabled: Boolean(resolved.data?.entity.blueprint_id),
  });
  const changes = useQuery({
    queryKey: entityQueryKeys.changes(entityId),
    queryFn: () => getEntityChanges(entityId),
    enabled: tab === 'changes',
  });
  const detailView = blueprint.data?.blueprint.views.detail;
  const heading = findEntityHeading(detailView);
  const HeadingRenderer = resolveHeadingRenderer(heading?.component);
  return (
    <PageContainer maxWidth="lg">
      <PageHeader eyebrow="Entity preview" />
      {resolved.data && blueprint.data && HeadingRenderer
        ? createElement(HeadingRenderer, {
            attributes: blueprint.data.attributes,
            entityId,
            values: resolved.data.values,
            view: detailView,
          })
        : null}
      <Box sx={{ mt: 1 }}>
        <Link params={{ entityId }} to="/entities/$entityId/edit">
          Edit entity
        </Link>
        {currentBlueprint.data && resolved.data && (
          <>
            {' | '}
            {currentBlueprint.data.blueprint.version >
            (resolved.data.entity.blueprint_version ?? Infinity) ? (
              <>
                <Typography
                  color="warning.main"
                  component="span"
                  sx={{
                    display: 'inline-flex',
                    gap: 0.5,
                    verticalAlign: 'middle',
                  }}
                >
                  <WarningAmberOutlinedIcon fontSize="small" />
                  Schema is outdated
                </Typography>
                {' | '}
                <Link params={{ entityId }} to="/entities/$entityId/migrate">
                  Upgrade blueprint
                </Link>
              </>
            ) : (
              <Typography
                component="span"
                sx={{
                  display: 'inline-flex',
                  gap: 0.5,
                  verticalAlign: 'middle',
                }}
              >
                <CheckCircleOutlinedIcon color="success" fontSize="small" />
                Matches current schema
              </Typography>
            )}
          </>
        )}
      </Box>
      <Tabs
        aria-label="Entity detail tabs"
        onChange={(_, value: 'preview' | 'changes') => setTab(value)}
        sx={{ mt: 2 }}
        value={tab}
      >
        <Tab label="Preview" value="preview" />
        <Tab label="Changes" value="changes" />
      </Tabs>
      {tab === 'changes' && changes.isPending && (
        <Typography sx={{ py: 3 }}>Loading changes...</Typography>
      )}
      {tab === 'changes' && changes.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>{changes.error.message}</Alert>
      )}
      {tab === 'changes' && changes.data && (
        <Box sx={{ mt: 3 }}>
          {Object.entries(groupChangesByEvent(changes.data)).map(([
            eventId,
            eventChanges,
          ]) => {
            const event = eventChanges[0];
            const actor = event.actor_display_name ?? event.actor_email ??
              (event.executor_type === 'agent' ? 'Agent' : 'Unknown actor');
            return (
              <Paper component="section" key={eventId} sx={{ mb: 2, p: 2 }}>
                <Typography sx={{ fontWeight: 'bold' }}>
                  {actor} · {new Date(event.occurred_at).toLocaleString()}
                </Typography>
                <Typography color="text.secondary" variant="body2">
                  {event.approval_decision
                    ? `Approval: ${event.approval_decision}${event.approved_by_display_name ? ` by ${event.approved_by_display_name}` : ''}`
                    : 'No approval attribution'}
                </Typography>
                {eventChanges.map((change) => (
                  <Box key={`${change.attribute_id}-${change.change_kind}`} sx={{ mt: 1 }}>
                    <Typography variant="body2">
                      <strong>{change.attribute_code}</strong>
                      {change.context_code ? ` (${change.context_code})` : ''}: {change.change_kind.replaceAll('_', ' ')}
                    </Typography>
                    <Typography component="pre" sx={{ fontFamily: 'monospace', m: 0, whiteSpace: 'pre-wrap' }} variant="body2">
                      {JSON.stringify(change.before_value)} → {JSON.stringify(change.after_value)}
                    </Typography>
                  </Box>
                ))}
              </Paper>
            );
          })}
          {changes.data.length === 0 && <Typography>No recorded changes.</Typography>}
        </Box>
      )}
      {tab === 'preview' && contexts.isPending && (
        <Typography sx={{ py: 3 }}>Loading contexts...</Typography>
      )}
      {tab === 'preview' && contexts.data && (
        <>
          <TextField
            select
            fullWidth
            label="Context"
            onChange={(event) => setSelectedContext(event.target.value)}
            sx={{ mt: 3 }}
            value={selectedContext}
          >
            {(contexts.data ?? []).map((context) => (
              <MenuItem key={context.id} value={context.code}>
                {context.code === 'default' ? 'Default' : context.code}
              </MenuItem>
            ))}
          </TextField>
          {resolved.isPending && (
            <Typography sx={{ mt: 3 }}>Resolving values...</Typography>
          )}
          {resolved.isError && (
            <Alert severity="error" sx={{ mt: 3 }}>
              {resolved.error.message}
            </Alert>
          )}
          {blueprint.isError && (
            <Alert severity="error" sx={{ mt: 3 }}>
              {blueprint.error.message}
            </Alert>
          )}
          {resolved.data && blueprint.data && (
            <Paper component="section" sx={{ mt: 3, p: { xs: 2, md: 3 } }}>
              <EntityView
                attributes={blueprint.data.attributes}
                contextId={selectedContextId}
                entityId={entityId}
                values={resolved.data.values}
                view={detailView}
                skipComponentId={entityHeadingComponentId}
              />
            </Paper>
          )}
        </>
      )}
    </PageContainer>
  );
};
