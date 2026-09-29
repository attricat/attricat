import { Button, Paper, TextField, Typography } from '@mui/material';
import { useForm } from '@tanstack/react-form';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { ErrorNotice } from './ExtensionErrorNotice';
import { invalidateExtensions } from './extensionPageUtils';
import {
  configureExtension,
  type ExtensionInstallation,
} from './managementApi';

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
    onSuccess: () => invalidateExtensions(client, extensionId),
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
    <Paper sx={{ p: 2 }}>
      <Typography variant="h6">{t('extensions.configuration')}</Typography>
      <Typography color="text.secondary" variant="body2">
        {t('extensions.serverValidation')}
      </Typography>
      <ErrorNotice error={config.error} />
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
            <TextField
              error={field.state.meta.errors.length > 0}
              fullWidth
              helperText={field.state.meta.errors[0]}
              label={t('extensions.jsonConfiguration')}
              minRows={5}
              multiline
              onBlur={field.handleBlur}
              onChange={(event) => field.handleChange(event.target.value)}
              sx={{ mt: 2 }}
              value={field.state.value}
            />
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
