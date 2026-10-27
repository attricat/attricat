import { useForm } from '@tanstack/react-form';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Box,
  Button,
  CircularProgress,
  FormControl,
  FormControlLabel,
  FormLabel,
  Radio,
  RadioGroup,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  getAgentSettings,
  updateAgentSettings,
  type AgentSettings,
} from './api';
import {
  bulkEditLimitFieldWidth,
  bulkEditPolicies,
  maxBulkEditMaxRecords,
} from './constants';
import { workspaceQueryKeys } from './queryKeys';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';

const AgentSettingsForm = ({ settings }: { settings: AgentSettings }) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [notice, setNotice] = useState('');
  const save = useMutation({
    meta: { toast: false },
    mutationFn: updateAgentSettings,
    onSuccess: (saved) => {
      queryClient.setQueryData(workspaceQueryKeys.agentSettings(), saved);
      setNotice(t('workspace.agentSettingsSaved'));
    },
  });
  const validateLimit = (value: string) => {
    const limit = Number(value);
    return Number.isInteger(limit) &&
      limit >= 1 &&
      limit <= maxBulkEditMaxRecords
      ? undefined
      : t('workspace.bulkEditLimitInvalid', { max: maxBulkEditMaxRecords });
  };
  const form = useForm({
    defaultValues: {
      bulk_edits: settings.bulk_edits,
      bulk_edit_max_records: String(settings.bulk_edit_max_records),
    },
    onSubmit: ({ value }) => {
      setNotice('');
      save.mutate({
        bulk_edits: value.bulk_edits,
        bulk_edit_max_records: Number(value.bulk_edit_max_records),
      });
    },
  });
  return (
    <Box
      component="form"
      noValidate
      onSubmit={(event) => {
        event.preventDefault();
        void form.handleSubmit();
      }}
    >
      <Stack spacing={2.5}>
        <form.Field name="bulk_edits">
          {(field) => (
            <FormControl disabled={save.isPending}>
              <FormLabel id="bulk-edit-policy">
                {t('workspace.bulkEditPolicy')}
              </FormLabel>
              <RadioGroup
                aria-labelledby="bulk-edit-policy"
                name={field.name}
                onChange={(event) =>
                  field.handleChange(
                    event.target.value as AgentSettings['bulk_edits'],
                  )
                }
                value={field.state.value}
              >
                {bulkEditPolicies.map((policy) => (
                  <FormControlLabel
                    control={<Radio />}
                    key={policy}
                    label={
                      <Box sx={{ py: 0.75 }}>
                        <Typography>
                          {t(`workspace.bulkEditPolicies.${policy}.label`)}
                        </Typography>
                        <Typography color="text.secondary" variant="body2">
                          {t(
                            `workspace.bulkEditPolicies.${policy}.description`,
                          )}
                        </Typography>
                      </Box>
                    }
                    sx={{ alignItems: 'flex-start', mt: 0.5 }}
                    value={policy}
                  />
                ))}
              </RadioGroup>
            </FormControl>
          )}
        </form.Field>
        <form.Field
          name="bulk_edit_max_records"
          validators={{
            onChange: ({ value }) => validateLimit(value),
            onSubmit: ({ value }) => validateLimit(value),
          }}
        >
          {(field) => (
            <TextField
              disabled={save.isPending}
              error={field.state.meta.errors.length > 0}
              helperText={
                field.state.meta.errors[0] ?? t('workspace.bulkEditLimitHelp')
              }
              label={t('workspace.bulkEditLimit')}
              name={field.name}
              onBlur={field.handleBlur}
              onChange={(event) => field.handleChange(event.target.value)}
              slotProps={{
                htmlInput: { inputMode: 'numeric', min: 1, step: 1 },
              }}
              sx={{ maxWidth: bulkEditLimitFieldWidth }}
              type="number"
              value={field.state.value}
            />
          )}
        </form.Field>
        <Box>
          <Button disabled={save.isPending} type="submit" variant="contained">
            {t('workspace.saveAgentSettings')}
          </Button>
        </Box>
        <Typography role="status" variant="body2">
          {notice}
        </Typography>
        <ApiErrorAlert error={save.error} />
      </Stack>
    </Box>
  );
};

export const WorkspaceAgentSection = ({
  canManage,
}: {
  canManage: boolean;
}) => {
  const { t } = useTranslation();
  const settings = useQuery({
    queryKey: workspaceQueryKeys.agentSettings(),
    queryFn: getAgentSettings,
    enabled: canManage,
  });
  if (!canManage)
    return (
      <Alert severity="error">
        {t('workspace.notAuthorizedAgentSettings')}
      </Alert>
    );
  return (
    <Stack spacing={2} sx={{ mt: 3 }}>
      <Typography component="h2" variant="h6">
        {t('workspace.agentSettingsTitle')}
      </Typography>
      <Typography color="text.secondary">
        {t('workspace.agentSettingsDescription')}
      </Typography>
      {settings.isPending && (
        <CircularProgress aria-label={t('workspace.loadingAgentSettings')} />
      )}
      <ApiErrorAlert error={settings.error} />
      {settings.data && <AgentSettingsForm settings={settings.data} />}
    </Stack>
  );
};
