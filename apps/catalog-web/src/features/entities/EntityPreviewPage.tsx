import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import CheckCircleOutlinedIcon from '@mui/icons-material/CheckCircleOutlined';
import WarningAmberOutlinedIcon from '@mui/icons-material/WarningAmberOutlined';
import {
  Alert,
  Box,
  Button,
  Container,
  MenuItem,
  Paper,
  TextField,
  Typography,
} from '@mui/material';
import { createElement, useState } from 'react';
import {
  getBlueprintRevision,
  getCurrentBlueprint,
  getEntityPreview,
  getResolvedEntityPreview,
  listContexts,
} from './api';
import { entityQueryKeys } from './query-keys';
import { EntityView } from '../views/components/EntityView';
import {
  entityHeadingComponent,
  findEntityHeading,
} from '../views/components/blocks/EntityHeading';
import { resolveHeadingRenderer } from '../views/components/registry';

export const EntityPreviewPage = ({ entityId }: { entityId: string }) => {
  const [selectedContext, setSelectedContext] = useState('default');
  const preview = useQuery({
    queryKey: entityQueryKeys.preview(entityId),
    queryFn: () => getEntityPreview(entityId),
  });
  const contexts = useQuery({
    queryKey: entityQueryKeys.contexts(),
    queryFn: listContexts,
  });
  const blueprint = useQuery({
    queryKey:
      preview.data?.entity.blueprint_id && preview.data.entity.blueprint_version
        ? entityQueryKeys.blueprintRevision(
            preview.data.entity.blueprint_id,
            preview.data.entity.blueprint_version,
          )
        : ['blueprint-revision'],
    queryFn: () =>
      getBlueprintRevision(
        preview.data!.entity.blueprint_id!,
        preview.data!.entity.blueprint_version!,
      ),
    enabled: Boolean(
      preview.data?.entity.blueprint_id &&
      preview.data.entity.blueprint_version,
    ),
  });
  const currentBlueprint = useQuery({
    queryKey: entityQueryKeys.currentBlueprint(
      preview.data?.entity.blueprint_id ?? '',
    ),
    queryFn: () => getCurrentBlueprint(preview.data!.entity.blueprint_id!),
    enabled: Boolean(preview.data?.entity.blueprint_id),
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
  const detailView = blueprint.data?.blueprint.views.detail;
  const heading = findEntityHeading(detailView);
  const HeadingRenderer = resolveHeadingRenderer(heading?.component);
  return (
    <Container component="main" maxWidth="lg" sx={{ py: { xs: 4, md: 7 } }}>
      <Button component={Link} to="/" sx={{ mb: 4 }}>
        Back to explorer
      </Button>
      <Typography
        color="primary"
        sx={{
          fontWeight: 700,
          letterSpacing: '.12em',
          textTransform: 'uppercase',
        }}
        variant="overline"
      >
        Entity preview
      </Typography>
      {resolved.data && blueprint.data && HeadingRenderer ? (
        createElement(HeadingRenderer, {
          attributes: blueprint.data.attributes,
          entityId,
          values: resolved.data.values,
          view: detailView,
        })
      ) : (
        <Typography component="h1" variant="h3">
          {entityId}
        </Typography>
      )}
      <Box sx={{ mt: 1 }}>
        <Link params={{ entityId }} to="/entities/$entityId/edit">
          Edit entity
        </Link>
        {currentBlueprint.data && preview.data && (
          <>
            {' | '}
            {currentBlueprint.data.blueprint.version >
            preview.data.entity.blueprint_version ? (
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
      {preview.isPending && (
        <Typography sx={{ py: 3 }}>Loading preview...</Typography>
      )}
      {preview.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {preview.error.message}
        </Alert>
      )}
      {preview.data && (
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
                values={resolved.data.values}
                view={detailView}
                skipComponentId={entityHeadingComponent.id}
              />
            </Paper>
          )}
        </>
      )}
    </Container>
  );
};
