import { useForm } from '@tanstack/react-form';
import { useNavigate } from '@tanstack/react-router';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, Button, Stack, TextField, Typography } from '@mui/material';
import { ArrowLeftIcon } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { SettingsPage } from '../../components/CenteredPage';
import { PageHeader } from '../../components/PageHeader';
import {
  PermissionIcon,
  PersonalTokenIcon,
} from '../../components/systemIcons';
import { futureZonedInstant } from '../../time/instantFormat';
import { useTimeZone } from '../../time/useInstantFormat';
import { useToast } from '../../components/useToast';
import { useCan } from '../auth/useCan';
import {
  tokenFormMaxWidth,
  tokenLabelMaxLength,
  tokensPath,
} from './constants';
import { ProfileSection } from './ProfileSection';
import { SecretDialog } from './SecretDialog';
import { createToken, listTokenPermissions } from './api';
import { profileQueryKeys } from './queryKeys';
import { useInitialFocus } from '../../components/initialFocus';
import { capabilities } from '../auth/capabilities';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { useExclusiveMutation } from '../../components/useExclusiveMutation';
import { PermissionChecklist } from './PermissionChecklist';
import { TokenPresetCards } from './TokenPresetCards';
import { RouterButton } from '../../components/RouterLink';

const BackToTokens = () => {
  const { t } = useTranslation();
  return (
    <RouterButton
      startIcon={<ArrowLeftIcon />}
      sx={{ alignSelf: 'flex-start', ml: -3.5 }}
      to={tokensPath}
    >
      {t('profile.tokenList')}
    </RouterButton>
  );
};

export const CreatePersonalTokenPage = () => {
  const { t } = useTranslation();
  const formRef = useInitialFocus<HTMLFormElement>();
  const { show } = useToast();
  const navigate = useNavigate();
  const client = useQueryClient();
  const canManage = useCan(capabilities.tokensManage);
  const permissions = useQuery({
    enabled: canManage,
    queryKey: profileQueryKeys.tokenPermissions(),
    queryFn: listTokenPermissions,
  });
  const [secret, setSecret] = useState<string>();
  const timeZone = useTimeZone();
  const create = useExclusiveMutation({
    meta: { toast: false },
    mutationFn: async (value: {
      label: string;
      permissions: string[];
      expires_at: string;
    }) => {
      const expiresAt = value.expires_at
        ? futureZonedInstant(value.expires_at, timeZone)
        : undefined;
      if (value.expires_at && !expiresAt)
        throw new Error(t('profile.tokenExpiry'));
      return createToken({
        label: value.label.trim(),
        permissions: value.permissions,
        ...(expiresAt ? { expires_at: expiresAt.toISOString() } : {}),
      });
    },
    onSuccess: (token) => {
      setSecret(token.secret);
      show({ message: t('profile.tokenCreated'), severity: 'success' });
      void client.invalidateQueries({ queryKey: profileQueryKeys.tokens() });
    },
  });
  const form = useForm({
    defaultValues: { label: '', permissions: [] as string[], expires_at: '' },
    onSubmit: ({ value }) => create.submit(value),
  });

  if (!canManage) {
    return (
      <SettingsPage>
        <Stack spacing={4} sx={{ maxWidth: tokenFormMaxWidth, mx: 'auto' }}>
          <BackToTokens />
          <PageHeader
            icon={PersonalTokenIcon}
            title={t('profile.createToken')}
          />
          <Alert severity="info">{t('profile.tokenUnavailable')}</Alert>
        </Stack>
      </SettingsPage>
    );
  }

  return (
    <SettingsPage>
      <SecretDialog
        onClose={() => void navigate({ to: tokensPath })}
        secret={secret}
      />
      <Stack
        ref={formRef}
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          form.handleSubmit();
        }}
        spacing={6}
        sx={{ maxWidth: tokenFormMaxWidth, mx: 'auto' }}
      >
        <Stack spacing={2}>
          <BackToTokens />
          <PageHeader
            description={t('profile.createTokenDescription')}
            icon={PersonalTokenIcon}
            title={t('profile.createToken')}
          />
        </Stack>
        <ApiErrorAlert error={create.error} />
        <ApiErrorAlert error={permissions.error} />
        <ProfileSection
          description={t('profile.tokenDetailsDescription')}
          icon={PersonalTokenIcon}
          title={t('profile.tokenDetailsTitle')}
        >
          <Stack spacing={5}>
            <form.Field name="label">
              {(field) => (
                <TextField
                  required
                  label={t('profile.label')}
                  onChange={(event) => field.handleChange(event.target.value)}
                  placeholder={t('profile.labelPlaceholder')}
                  slotProps={{ htmlInput: { maxLength: tokenLabelMaxLength } }}
                  value={field.state.value}
                />
              )}
            </form.Field>
            <form.Field name="expires_at">
              {(field) => (
                <TextField
                  helperText={t('profile.expiresAtHelp', { zone: timeZone })}
                  label={t('profile.expiresAt')}
                  onChange={(event) => field.handleChange(event.target.value)}
                  slotProps={{ inputLabel: { shrink: true } }}
                  type="datetime-local"
                  value={field.state.value}
                />
              )}
            </form.Field>
          </Stack>
        </ProfileSection>
        <form.Field name="permissions">
          {(field) => {
            const selected = field.state.value;
            const toggle = (code: string, checked: boolean) =>
              field.handleChange(
                checked
                  ? [...selected, code]
                  : selected.filter((item) => item !== code),
              );
            return (
              <ProfileSection
                description={t('profile.permissionsDescription')}
                icon={PermissionIcon}
                title={t('profile.permissions')}
              >
                <Typography component="h3" variant="subtitle1">
                  {t('profile.permissionPresets')}
                </Typography>
                <TokenPresetCards
                  available={
                    permissions.data &&
                    new Set(
                      permissions.data.map((permission) => permission.code),
                    )
                  }
                  onSelect={field.handleChange}
                  selected={selected}
                />
                <Stack
                  direction="row"
                  spacing={2}
                  sx={{
                    alignItems: 'baseline',
                    justifyContent: 'space-between',
                    mb: 3,
                    mt: 6,
                  }}
                >
                  <Typography component="h3" variant="subtitle1">
                    {t('profile.customPermissions')}
                  </Typography>
                  <Typography
                    aria-live="polite"
                    color="text.secondary"
                    variant="caption"
                  >
                    {t('profile.selectedPermissions', {
                      count: selected.length,
                    })}
                  </Typography>
                </Stack>
                <PermissionChecklist
                  onToggle={toggle}
                  permissions={permissions.data ?? []}
                  selected={selected}
                />
              </ProfileSection>
            );
          }}
        </form.Field>
        <Stack direction="row" spacing={3} sx={{ justifyContent: 'flex-end' }}>
          <RouterButton to={tokensPath} variant="outlined">
            {t('common.cancel')}
          </RouterButton>
          <Button disabled={create.isPending} type="submit" variant="contained">
            {t('profile.create')}
          </Button>
        </Stack>
      </Stack>
    </SettingsPage>
  );
};
