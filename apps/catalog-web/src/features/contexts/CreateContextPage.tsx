import { useForm } from '@tanstack/react-form';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link, useNavigate } from '@tanstack/react-router';
import {
  Alert,
  Button,
  Paper,
  Stack,
  MenuItem,
  TextField,
} from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { QueryErrorNotice } from '../../components/QueryErrorNotice';
import { createContext, listContexts } from './api';
import { contextQueryKeys } from './query-keys';

export const CreateContextPage = () => {
  const { t } = useTranslation();
  const navigate = useNavigate({ from: '/manage/contexts/new' });
  const queryClient = useQueryClient();
  const [validationError, setValidationError] = useState<string>();
  const contexts = useQuery({
    queryKey: contextQueryKeys.all(),
    queryFn: ({ signal }) => listContexts(signal),
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
        queryKey: contextQueryKeys.all(),
      });
      void navigate({ to: '/manage/contexts' });
    },
  });
  const form = useForm({
    defaultValues: { code: '', data: '{}', parentId: '' },
    onSubmit: ({ value }) => {
      setValidationError(undefined);
      let data: unknown;
      try {
        data = JSON.parse(value.data);
      } catch {
        setValidationError(t('contexts.invalidMetadataJson'));
        return;
      }
      if (typeof data !== 'object' || data === null || Array.isArray(data)) {
        setValidationError(t('contexts.metadataMustBeObject'));
        return;
      }
      create.mutate({
        code: value.code.trim(),
        data: data as Record<string, unknown>,
        parentId: value.parentId,
      });
    },
  });
  return (
    <PageContainer>
      <Button component={Link} to="/manage/contexts" sx={{ mb: 4 }}>
        {t('contexts.backToContexts')}
      </Button>
      <PageHeader title={t('contexts.createContext')} titleVariant="h3" />
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
                label={t('contexts.code')}
                onChange={(event) => field.handleChange(event.target.value)}
                required
                value={field.state.value}
              />
            )}
          </form.Field>
          <QueryErrorNotice
            error={contexts.error}
            isRetrying={contexts.isFetching}
            onRetry={() => void contexts.refetch()}
          />
          <form.Field name="parentId">
            {(field) => (
              <TextField
                disabled={contexts.isPending || contexts.isError}
                helperText={
                  contexts.isPending ? t('contexts.loadingContexts') : undefined
                }
                select
                label={t('contexts.parentContext')}
                required
                value={field.state.value}
                onChange={(event) => field.handleChange(event.target.value)}
              >
                <MenuItem value="">{t('contexts.selectParent')}</MenuItem>
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
                label={t('contexts.metadata')}
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
          <Button
            disabled={
              create.isPending || contexts.isPending || contexts.isError
            }
            type="submit"
            variant="contained"
          >
            {t('contexts.createContext')}
          </Button>
        </Stack>
      </Paper>
    </PageContainer>
  );
};
