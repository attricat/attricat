import { useForm } from '@tanstack/react-form';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { Alert, Button, Paper, Typography } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/query-keys';
import { sideloadExtension } from './management-api';
import { ErrorNotice } from './ExtensionErrorNotice';
import { invalidateExtensions } from './extension-page-utils';

export const SideloadExtensionPage = () => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const sideload = useMutation({
    mutationFn: sideloadExtension,
    onSuccess: () => invalidateExtensions(client),
  });
  const [archive, setArchive] = useState<File | null>(null);
  const form = useForm({
    defaultValues: { archive: null as File | null },
    onSubmit: ({ value }) => {
      if (value.archive) sideload.mutate(value.archive);
    },
  });
  const canManage = session.data?.capabilities?.extensions_manage === true;
  if (session.data && !canManage)
    return (
      <PageContainer>
        <Alert severity="error">{t('extensions.notAuthorizedInstall')}</Alert>
      </PageContainer>
    );
  return (
    <PageContainer>
      <PageHeader
        title={t('extensions.uploadTitle')}
        description={t('extensions.uploadDescription')}
        actions={<Link to="/manage/extensions">{t('extensions.back')}</Link>}
      />
      <ErrorNotice error={sideload.error} />
      {sideload.isSuccess && (
        <Alert severity="success" sx={{ mb: 2 }}>
          {t('extensions.installedSuccess')}
        </Alert>
      )}
      <Paper sx={{ p: 2 }}>
        <form
          onSubmit={(event) => {
            event.preventDefault();
            void form.handleSubmit();
          }}
        >
          <form.Field name="archive">
            {(field) => (
              <Button component="label" variant="outlined">
                {field.state.value?.name ?? t('extensions.chooseArchive')}
                <input
                  accept=".tar.zst,application/zstd"
                  hidden
                  onChange={(event) => {
                    const archive = event.target.files?.[0] ?? null;
                    field.handleChange(archive);
                    setArchive(archive);
                  }}
                  type="file"
                />
              </Button>
            )}
          </form.Field>
          <Typography color="text.secondary" sx={{ mt: 1 }} variant="body2">
            {t('extensions.archiveHelp')}
          </Typography>
          <Button
            disabled={!canManage || !archive || sideload.isPending}
            sx={{ mt: 2 }}
            type="submit"
            variant="contained"
          >
            {sideload.isPending
              ? t('extensions.installing')
              : t('extensions.installArchive')}
          </Button>
        </form>
      </Paper>
    </PageContainer>
  );
};
