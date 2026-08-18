import { useForm } from '@tanstack/react-form';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link, useNavigate } from '@tanstack/react-router';
import {
  Alert,
  Button,
  Container,
  Paper,
  Stack,
  MenuItem,
  TextField,
  Typography,
} from '@mui/material';
import { useState } from 'react';
import { createContext, listContexts } from '../entities/api';
import { entityQueryKeys } from '../entities/query-keys';

export const CreateContextPage = () => {
  const navigate = useNavigate({ from: '/contexts/new' });
  const queryClient = useQueryClient();
  const [validationError, setValidationError] = useState<string>();
  const contexts = useQuery({
    queryKey: entityQueryKeys.contexts(),
    queryFn: listContexts,
  });
  const create = useMutation({
    mutationFn: ({
      code,
      data,
      parentId,
    }: {
      code: string;
      data: Record<string, unknown>;
      parentId: string;
    }) => createContext(code, data, parentId),
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: entityQueryKeys.contexts(),
      });
      void navigate({ to: '/contexts' });
    },
  });
  const form = useForm({
    defaultValues: { code: '', data: '{}', parentId: '' },
    onSubmit: ({ value }) => {
      setValidationError(undefined);
      try {
        const data: unknown = JSON.parse(value.data);
        if (typeof data !== 'object' || data === null || Array.isArray(data)) {
          throw new Error('Metadata must be a JSON object');
        }
        create.mutate({
          code: value.code.trim(),
          data: data as Record<string, unknown>,
          parentId: value.parentId,
        });
      } catch (error) {
        setValidationError(
          error instanceof Error ? error.message : 'Invalid metadata',
        );
      }
    },
  });
  return (
    <Container component="main" maxWidth="sm" sx={{ py: { xs: 4, md: 7 } }}>
      <Button component={Link} to="/contexts" sx={{ mb: 4 }}>
        Back to contexts
      </Button>
      <Typography component="h1" variant="h3">
        Create context
      </Typography>
      <Paper
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          void form.handleSubmit();
        }}
        sx={{ mt: 4, p: 3 }}
      >
        <Stack spacing={2}>
          <form.Field name="code">
            {(field) => (
              <TextField
                label="Code"
                onChange={(event) => field.handleChange(event.target.value)}
                required
                value={field.state.value}
              />
            )}
          </form.Field>
          <form.Field name="parentId">
            {(field) => (
              <TextField
                select
                label="Parent context"
                required
                value={field.state.value}
                onChange={(event) => field.handleChange(event.target.value)}
              >
                <MenuItem value="">Select a parent</MenuItem>
                {(contexts.data ?? []).map((context) => (
                  <MenuItem key={context.id} value={context.id}>
                    {context.code}
                  </MenuItem>
                ))}
              </TextField>
            )}
          </form.Field>
          <form.Field name="data">
            {(field) => (
              <TextField
                label="Metadata"
                multiline
                minRows={5}
                onChange={(event) => field.handleChange(event.target.value)}
                required
                value={field.state.value}
              />
            )}
          </form.Field>
          {(create.error || validationError) && (
            <Alert severity="error">
              {create.error?.message ?? validationError}
            </Alert>
          )}
          <Button disabled={create.isPending} type="submit" variant="contained">
            Create context
          </Button>
        </Stack>
      </Paper>
    </Container>
  );
};
