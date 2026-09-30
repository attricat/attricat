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
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { QueryErrorNotice } from '../../components/QueryErrorNotice';
import { createContext, listContexts } from './api';
import { contextMetadataMinRows, emptyContextMetadata } from './constants';
import { parseContextMetadata } from './contextMetadata';
import { contextQueryKeys } from './queryKeys';
import { ContextIcon } from '../../components/systemIcons';

export const CreateContextPage = () => {
  const { t } = useTranslation();
  const navigate = useNavigate({ from: '/manage/contexts/new' });
  const queryClient = useQueryClient();
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
    defaultValues: { code: '', data: emptyContextMetadata, parentId: '' },
    onSubmit: ({ value }) => {
      if (create.isPending) return;
      const metadata = parseContextMetadata(value.data);
      if (!metadata.data) return;
      create.mutate({
        code: value.code.trim(),
        data: metadata.data,
        parentId: value.parentId,
      });
    },
  });
  return (
    <PageContainer>
      <Button component={Link} to="/manage/contexts" sx={{ mb: 4 }}>
        {t('contexts.backToContexts')}
      </Button>
      <PageHeader
        icon={ContextIcon}
        title={t('contexts.createContext')}
        titleVariant="h3"
      />
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
                disabled={create.isPending}
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
                disabled={
                  create.isPending || contexts.isPending || contexts.isError
                }
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
          <form.Field
            name="data"
            validators={{
              onSubmit: ({ value }) => parseContextMetadata(value).errorKey,
            }}
          >
            {(field) => (
              <TextField
                disabled={create.isPending}
                error={field.state.meta.errors.length > 0}
                helperText={
                  field.state.meta.errors[0]
                    ? t(field.state.meta.errors[0])
                    : undefined
                }
                label={t('contexts.metadata')}
                multiline
                minRows={contextMetadataMinRows}
                onChange={(event) => field.handleChange(event.target.value)}
                required
                value={field.state.value}
              />
            )}
          </form.Field>
          {create.error && (
            <Alert severity="error">{create.error.message}</Alert>
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
