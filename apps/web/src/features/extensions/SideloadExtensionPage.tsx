import { useForm } from '@tanstack/react-form';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { Alert, Button, Paper, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { UploadIcon } from 'lucide-react';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { sideloadExtension } from './managementApi';
import { invalidateExtensions } from './extensionPageUtils';
import { extensionArchiveAccept } from './constants';
import { ExtensionIcon } from '../../components/systemIcons';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';

export const SideloadExtensionPage = () => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const sideload = useMutation({
    mutationFn: sideloadExtension,
    onSuccess: () => invalidateExtensions(client),
  });
  const form = useForm({
    defaultValues: { archive: null as File | null },
    onSubmit: ({ value }) => {
      if (value.archive) sideload.mutate(value.archive);
    },
  });
  return (
    <PageContainer>
      <PageHeader
        icon={ExtensionIcon}
        title={t('extensions.uploadTitle')}
        description={t('extensions.uploadDescription')}
        actions={<Link to="/manage/extensions">{t('extensions.back')}</Link>}
      />
      <ApiErrorAlert error={sideload.error} sx={{ mb: 2 }} />
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
              <Button
                component="label"
                startIcon={<UploadIcon />}
                variant="outlined"
              >
                {field.state.value?.name ?? t('extensions.chooseArchive')}
                <input
                  accept={extensionArchiveAccept}
                  hidden
                  onChange={(event) =>
                    field.handleChange(event.target.files?.[0] ?? null)
                  }
                  type="file"
                />
              </Button>
            )}
          </form.Field>
          <Typography color="text.secondary" sx={{ mt: 1 }} variant="body2">
            {t('extensions.archiveHelp')}
          </Typography>
          <form.Subscribe selector={(state) => state.values.archive}>
            {(archive) => (
              <Button
                disabled={!archive || sideload.isPending}
                sx={{ mt: 2 }}
                type="submit"
                variant="contained"
              >
                {sideload.isPending
                  ? t('extensions.installing')
                  : t('extensions.installArchive')}
              </Button>
            )}
          </form.Subscribe>
        </form>
      </Paper>
    </PageContainer>
  );
};
