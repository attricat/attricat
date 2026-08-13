import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
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
import { useState } from 'react';
import { getBlueprintRevision, getEntityPreview } from './api';
import { resolvePreviewContext } from './preview-context';
import { entityQueryKeys } from './query-keys';

export const EntityPreviewPage = ({ entityId }: { entityId: string }) => {
  const [selectedContext, setSelectedContext] = useState('default');
  const preview = useQuery({
    queryKey: entityQueryKeys.preview(entityId),
    queryFn: () => getEntityPreview(entityId),
  });
  const blueprint = useQuery({
    queryKey: preview.data
      ? entityQueryKeys.blueprintRevision(
          preview.data.entity.blueprint_id,
          preview.data.entity.blueprint_version,
        )
      : ['blueprint-revision'],
    queryFn: () =>
      getBlueprintRevision(
        preview.data!.entity.blueprint_id,
        preview.data!.entity.blueprint_version,
      ),
    enabled: Boolean(preview.data),
  });
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
      <Typography component="h1" variant="h3">
        {entityId}
      </Typography>
      <Box sx={{ mt: 1 }}>
        <Link params={{ entityId }} to="/entities/$entityId/edit">
          Edit entity
        </Link>
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
            {Object.keys(preview.data.context).map((context) => (
              <MenuItem key={context} value={context}>
                {context === 'default' ? 'Default' : context}
              </MenuItem>
            ))}
          </TextField>
          {blueprint.isPending && <Typography sx={{ mt: 3 }}>Loading schema...</Typography>}
          {blueprint.isError && <Alert severity="error" sx={{ mt: 3 }}>{blueprint.error.message}</Alert>}
          {blueprint.data && (
            <Paper component="pre" sx={{ mt: 3, overflow: 'auto', p: 3 }}>
              {JSON.stringify(
                resolvePreviewContext(
                  preview.data.context,
                  selectedContext,
                  blueprint.data.attributes,
                ),
                null,
                2,
              )}
            </Paper>
          )}
        </>
      )}
    </Container>
  );
};
