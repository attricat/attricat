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
import { getEntityPreview, getResolvedEntityPreview, listContexts } from './api';
import { entityQueryKeys } from './query-keys';

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
  const selectedContextId = contexts.data?.find((context) => context.code === selectedContext)?.id;
  const resolved = useQuery({
    queryKey: selectedContextId
      ? entityQueryKeys.resolvedPreview(entityId, selectedContextId)
      : ['entity-resolved-preview'],
    queryFn: () => getResolvedEntityPreview(entityId, selectedContextId!),
    enabled: Boolean(selectedContextId),
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
            {contexts.data?.map((context) => (
              <MenuItem key={context.id} value={context.code}>
                {context.code === 'default' ? 'Default' : context.code}
              </MenuItem>
            ))}
          </TextField>
            {resolved.isPending && <Typography sx={{ mt: 3 }}>Resolving values...</Typography>}
            {resolved.isError && <Alert severity="error" sx={{ mt: 3 }}>{resolved.error.message}</Alert>}
            {resolved.data && (
              <Paper component="pre" sx={{ mt: 3, overflow: 'auto', p: 3 }}>
                {JSON.stringify(
                  {
                    requested_context: resolved.data.requested_context,
                    values: resolved.data.values,
                  },
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
