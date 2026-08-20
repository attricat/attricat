import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import CheckCircleOutlinedIcon from '@mui/icons-material/CheckCircleOutlined';
import WarningAmberOutlinedIcon from '@mui/icons-material/WarningAmberOutlined';
import {
  Alert,
  Box,
  MenuItem,
  Paper,
  TextField,
  Typography,
} from '@mui/material';
import { createElement, useState } from 'react';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import {
  getBlueprintRevision,
  getCurrentBlueprint,
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

export const EntityPreviewPage = ({ entityId }: { entityId: string }) => {
  const [selectedContext, setSelectedContext] = useState('default');
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
            resolved.data.entity.blueprint_version ? (
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
      {contexts.isPending && (
        <Typography sx={{ py: 3 }}>Loading contexts...</Typography>
      )}
      {contexts.data && (
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
