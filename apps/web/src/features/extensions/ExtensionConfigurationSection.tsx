import { Box, Button, Paper, Skeleton, Typography } from '@mui/material';
import { useForm } from '@tanstack/react-form';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { lazy, Suspense } from 'react';
import { useTranslation } from 'react-i18next';
import { invalidateExtensions } from './extensionPageUtils';
import {
  configureExtension,
  type ExtensionInstallation,
} from './managementApi';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { jsonEditorMinHeight } from '../../components/jsonEditorSize';

// Monaco loads in its own chunk, only where configuration is shown.
const JsonEditor = lazy(() =>
  import('../../components/JsonEditor').then(({ JsonEditor }) => ({
    default: JsonEditor,
  })),
);

const configurationIndentation = 2;

const parseConfiguration = (draft: string) => {
  try {
    return { ok: true as const, value: JSON.parse(draft) as unknown };
  } catch {
    return { ok: false as const };
  }
};

type ExtensionConfigurationSectionProps = {
  canManage: boolean;
  extensionId: string;
  installation: ExtensionInstallation;
};

/**
 * Remount this section (for example with the installed release as its key) to
 * hydrate the draft from a newly installed release's configuration.
 */
export const ExtensionConfigurationSection = ({
  canManage,
  extensionId,
  installation,
}: ExtensionConfigurationSectionProps) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const config = useMutation({
    mutationFn: (value: unknown) => configureExtension(extensionId, value),
    onSuccess: () => invalidateExtensions(client),
  });
  const form = useForm({
    defaultValues: {
      configuration: JSON.stringify(
        installation.configuration,
        null,
        configurationIndentation,
      ),
    },
    onSubmit: ({ value }) => {
      const parsed = parseConfiguration(value.configuration);
      if (parsed.ok) config.mutate(parsed.value);
    },
  });
  const validateConfiguration = (draft: string) =>
    parseConfiguration(draft).ok
      ? undefined
      : t('extensions.configurationJsonError');
  return (
    <Paper sx={{ p: 4 }}>
      <Typography component="h2" variant="h6">
        {t('extensions.configuration')}
      </Typography>
      <Typography color="text.secondary" variant="body2">
        {t('extensions.serverValidation')}
      </Typography>
      <ApiErrorAlert error={config.error} sx={{ mb: 2 }} />
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void form.handleSubmit();
        }}
      >
        <form.Field
          name="configuration"
          validators={{
            onSubmit: ({ value }) => validateConfiguration(value),
          }}
        >
          {(field) => (
            <Suspense
              fallback={
                <Skeleton
                  height={jsonEditorMinHeight}
                  sx={{ mt: 2 }}
                  variant="rectangular"
                />
              }
            >
              <Box sx={{ mt: 2 }}>
                <JsonEditor
                  defaultValue={field.state.value}
                  error={field.state.meta.errors[0]}
                  label={t('extensions.jsonConfiguration')}
                  onBlur={field.handleBlur}
                  onChange={field.handleChange}
                  readOnly={!canManage}
                />
              </Box>
            </Suspense>
          )}
        </form.Field>
        <Button
          disabled={!canManage || config.isPending}
          sx={{ mt: 1 }}
          type="submit"
          variant="contained"
        >
          {t('extensions.saveConfiguration')}
        </Button>
      </form>
    </Paper>
  );
};
