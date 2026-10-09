import { useForm } from '@tanstack/react-form';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, Button, Paper, TextField, Typography } from '@mui/material';
import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { ErrorNotice } from './ExtensionErrorNotice';
import { invalidateExtensions } from './extensionPageUtils';
import {
  updateWorkspaceExtensionLayout,
  workspaceExtensionLayout,
} from './managementApi';
import { extensionManagementQueryKeys } from './managementQueryKeys';
import { workspaceExtensionLayoutSchema } from './schemas';

export const ExtensionsLayoutPage = () => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const layout = useQuery({
    queryKey: extensionManagementQueryKeys.layout(),
    queryFn: workspaceExtensionLayout,
    enabled: session.data?.capabilities?.extensions_manage === true,
  });
  const saveLayout = useMutation({
    mutationFn: updateWorkspaceExtensionLayout,
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: extensionManagementQueryKeys.layout(),
      });
      invalidateExtensions(queryClient);
    },
  });
  const parseLayout = (draft: string) => {
    try {
      return workspaceExtensionLayoutSchema.safeParse(JSON.parse(draft));
    } catch {
      return undefined;
    }
  };
  const validateLayout = (draft: string) =>
    parseLayout(draft)?.success ? undefined : t('extensions.layoutInvalid');
  const form = useForm({
    defaultValues: { layout: '' },
    onSubmit: ({ value }) => {
      const parsed = parseLayout(value.layout);
      if (parsed?.success) saveLayout.mutate(parsed.data);
    },
  });
  const hydratedLayout = useRef<string | undefined>(undefined);
  useEffect(() => {
    if (layout.data) {
      const serialized = JSON.stringify(layout.data, null, 2);
      if (hydratedLayout.current !== serialized) {
        // A background refetch must not overwrite an unsaved JSON draft.
        if (
          !form.state.isDirty ||
          form.state.values.layout === hydratedLayout.current ||
          form.state.values.layout === serialized
        ) {
          form.reset({ layout: serialized });
        }
        hydratedLayout.current = serialized;
      }
    }
  }, [form, layout.data]);

  if (session.data && !session.data.capabilities?.extensions_manage) {
    return (
      <Alert severity="error">{t('extensions.notAuthorizedManage')}</Alert>
    );
  }

  return (
    <>
      <ErrorNotice error={layout.error ?? saveLayout.error} />
      <Paper
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          form.handleSubmit();
        }}
        sx={{ p: 2 }}
      >
        <Typography variant="h5">{t('extensions.layout')}</Typography>
        <Typography color="text.secondary" sx={{ mb: 2 }}>
          {t('extensions.layoutDescription')}
        </Typography>
        <form.Field
          name="layout"
          validators={{
            onBlur: ({ value }) => validateLayout(value),
            onChange: ({ value }) => validateLayout(value),
            onSubmit: ({ value }) => validateLayout(value),
          }}
        >
          {(field) => (
            <TextField
              required
              error={field.state.meta.errors.length > 0}
              fullWidth
              helperText={field.state.meta.errors[0]}
              label={t('extensions.layout')}
              minRows={8}
              multiline
              onBlur={field.handleBlur}
              onChange={(event) => field.handleChange(event.target.value)}
              value={field.state.value}
            />
          )}
        </form.Field>
        <form.Subscribe
          selector={(state) => [state.canSubmit, state.values.layout]}
        >
          {([canSubmit, draft]) => (
            <Button
              disabled={!canSubmit || !draft || saveLayout.isPending}
              sx={{ mt: 2 }}
              type="submit"
            >
              {t('extensions.saveLayout')}
            </Button>
          )}
        </form.Subscribe>
      </Paper>
    </>
  );
};
