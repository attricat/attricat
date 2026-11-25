import { MenuItem, Stack, TextField } from '@mui/material';
import { useMutation } from '@tanstack/react-query';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { FormDialog } from '../../components/FormDialog';
import { createWebhookEndpoint } from './api';
import { webhookEndpointNamePattern } from './constants';
import type { IssuedWebhookEndpoint, ManifestWebhook } from './schemas';

/** Creates a public endpoint for one of the extension's declared webhooks. */
export const WebhookEndpointDialog = ({
  extensionId,
  onClose,
  onCreated,
  webhooks,
}: {
  extensionId: string;
  onClose: () => void;
  onCreated: (issued: IssuedWebhookEndpoint) => void;
  webhooks: ManifestWebhook[];
}) => {
  const { t } = useTranslation();
  const [webhook, setWebhook] = useState(webhooks[0]?.id ?? '');
  const [endpoint, setEndpoint] = useState('');
  // The name is checked once the person has tried to submit it.
  const [submitted, setSubmitted] = useState(false);
  const create = useMutation({
    mutationFn: () =>
      createWebhookEndpoint(extensionId, {
        webhook,
        endpoint: endpoint.trim(),
      }),
    onSuccess: onCreated,
  });
  const nameInvalid = !webhookEndpointNamePattern.test(endpoint.trim());
  return (
    <FormDialog
      description={t('extensionWebhooks.addEndpointDescription')}
      error={create.error}
      isPending={create.isPending}
      onClose={onClose}
      onSubmit={() => {
        setSubmitted(true);
        if (nameInvalid || !webhook) return;
        create.mutate();
      }}
      submitLabel={t('extensionWebhooks.create')}
      title={t('extensionWebhooks.addEndpoint')}
    >
      <Stack spacing={2} sx={{ mt: 2 }}>
        <TextField
          label={t('extensionWebhooks.webhook')}
          required
          select
          value={webhook}
          onChange={(event) => setWebhook(event.target.value)}
        >
          {webhooks.map((item) => (
            <MenuItem key={item.id} value={item.id}>
              {item.title}
            </MenuItem>
          ))}
        </TextField>
        <TextField
          error={submitted && nameInvalid}
          helperText={
            submitted && nameInvalid
              ? t('extensionWebhooks.endpointNameInvalid')
              : t('extensionWebhooks.endpointNameHelp')
          }
          label={t('extensionWebhooks.endpointName')}
          required
          value={endpoint}
          onChange={(event) => setEndpoint(event.target.value)}
        />
      </Stack>
    </FormDialog>
  );
};
