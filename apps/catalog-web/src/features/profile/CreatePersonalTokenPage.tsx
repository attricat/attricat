import { useForm } from '@tanstack/react-form';
import { Link, useNavigate } from '@tanstack/react-router';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Box,
  Button,
  Card,
  CardActionArea,
  Checkbox,
  FormControlLabel,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { ArrowLeftIcon, CircleCheckIcon } from 'lucide-react';
import { useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { monoFontFamily } from '../../app/theme';
import { SettingsPage } from '../../components/CenteredPage';
import { PageHeader } from '../../components/PageHeader';
import { compactIconSize } from '../../components/iconSizes';
import {
  PermissionIcon,
  PersonalTokenIcon,
} from '../../components/systemIcons';
import { zonedDateTimeToIso } from '../../time/instantFormat';
import { useTimeZone } from '../../time/useInstantFormat';
import { useToast } from '../../components/useToast';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import {
  tokenFormMaxWidth,
  tokenPermissionPresets,
  tokensPath,
} from './constants';
import { ProfileSection } from './ProfileSection';
import { SecretDialog } from './SecretDialog';
import { createToken, listTokenPermissions } from './api';
import { profileQueryKeys } from './queryKeys';

const permissionGroup = (code: string) => code.split('.')[0] ?? code;

const groupPermissions = <T extends { code: string }>(permissions: T[]) =>
  permissions.reduce((groups, permission) => {
    const group = permissionGroup(permission.code);
    groups.set(group, [...(groups.get(group) ?? []), permission]);
    return groups;
  }, new Map<string, T[]>());

const sameCodes = (left: readonly string[], right: readonly string[]) =>
  left.length === right.length && left.every((code) => right.includes(code));

const BackToTokens = () => {
  const { t } = useTranslation();
  return (
    <Button
      component={Link}
      startIcon={<ArrowLeftIcon />}
      sx={{ alignSelf: 'flex-start', ml: -3.5 }}
      to={tokensPath}
    >
      {t('profile.tokenList')}
    </Button>
  );
};

export const CreatePersonalTokenPage = () => {
  const { t } = useTranslation();
  const { show } = useToast();
  const navigate = useNavigate();
  const client = useQueryClient();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const canManage = session.data?.capabilities?.tokens_manage === true;
  const permissions = useQuery({
    enabled: canManage,
    queryKey: profileQueryKeys.tokenPermissions(),
    queryFn: listTokenPermissions,
  });
  const [secret, setSecret] = useState<string>();
  const creating = useRef(false);
  const [isCreating, setIsCreating] = useState(false);
  const [error, setError] = useState<string>();
  const timeZone = useTimeZone();
  const permissionIdPrefix = useId();
  const form = useForm({
    defaultValues: { label: '', permissions: [] as string[], expires_at: '' },
    onSubmit: async ({ value }) => {
      if (creating.current) return;
      creating.current = true;
      setIsCreating(true);
      setError(undefined);
      try {
        const expiresAt = value.expires_at
          ? new Date(zonedDateTimeToIso(value.expires_at, timeZone))
          : undefined;
        if (
          expiresAt &&
          (Number.isNaN(expiresAt.getTime()) || expiresAt <= new Date())
        ) {
          throw new Error(t('profile.tokenExpiry'));
        }
        const token = await createToken({
          label: value.label,
          permissions: value.permissions,
          ...(expiresAt ? { expires_at: expiresAt.toISOString() } : {}),
        });
        setSecret(token.secret);
        show({ message: t('profile.tokenCreated'), severity: 'success' });
        void client.invalidateQueries({ queryKey: profileQueryKeys.tokens() });
      } catch (reason) {
        setError(
          reason instanceof Error ? reason.message : t('profile.createFailed'),
        );
      } finally {
        creating.current = false;
        setIsCreating(false);
      }
    },
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
        {error && <Alert severity="error">{error}</Alert>}
        {permissions.isError && (
          <Alert severity="error">{permissions.error.message}</Alert>
        )}
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
            const availablePermissions = new Set(
              permissions.data?.map((permission) => permission.code),
            );
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
                <Box
                  sx={{
                    display: 'grid',
                    gap: 3,
                    gridTemplateColumns: {
                      xs: '1fr',
                      sm: 'repeat(3, minmax(0, 1fr))',
                    },
                    mt: 3,
                  }}
                >
                  {tokenPermissionPresets.map((preset) => {
                    const unavailable = permissions.data
                      ? preset.permissions.some(
                          (code) => !availablePermissions.has(code),
                        )
                      : false;
                    const active = sameCodes(selected, preset.permissions);
                    return (
                      <Card
                        key={preset.nameKey}
                        sx={{
                          borderColor: active ? 'primary.main' : undefined,
                          bgcolor: active ? 'action.selected' : undefined,
                        }}
                      >
                        <CardActionArea
                          aria-pressed={active}
                          disabled={!permissions.data || unavailable}
                          onClick={() => field.handleChange(preset.permissions)}
                          sx={{ height: '100%', p: 4 }}
                        >
                          <Stack
                            direction="row"
                            spacing={2}
                            sx={{
                              alignItems: 'center',
                              justifyContent: 'space-between',
                            }}
                          >
                            <Typography
                              color={unavailable ? 'text.disabled' : undefined}
                              variant="subtitle1"
                            >
                              {t(preset.nameKey)}
                            </Typography>
                            {active && (
                              <Box
                                sx={{ color: 'primary.main', display: 'flex' }}
                              >
                                <CircleCheckIcon
                                  aria-hidden
                                  size={compactIconSize}
                                />
                              </Box>
                            )}
                          </Stack>
                          <Typography
                            color="text.secondary"
                            component="p"
                            sx={{ mt: 1 }}
                            variant="caption"
                          >
                            {unavailable
                              ? t('profile.unavailable')
                              : t(preset.descriptionKey)}
                          </Typography>
                          <Typography
                            color="text.secondary"
                            component="p"
                            sx={{ mt: 2 }}
                            variant="caption"
                          >
                            {t('profile.permissionCount', {
                              count: preset.permissions.length,
                            })}
                          </Typography>
                        </CardActionArea>
                      </Card>
                    );
                  })}
                </Box>
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
                <Box
                  sx={{
                    display: 'grid',
                    gap: 5,
                    gridTemplateColumns: {
                      xs: '1fr',
                      sm: 'repeat(2, minmax(0, 1fr))',
                    },
                  }}
                >
                  {[...groupPermissions(permissions.data ?? [])].map(
                    ([group, items]) => (
                      <Box
                        component="fieldset"
                        key={group}
                        sx={{ border: 0, m: 0, p: 0 }}
                      >
                        <Typography
                          color="text.secondary"
                          component="legend"
                          sx={{ fontFamily: monoFontFamily, mb: 1 }}
                          variant="overline"
                        >
                          {group}
                        </Typography>
                        {items.map((permission) => {
                          const idPrefix = `${permissionIdPrefix}-${permission.code}`;
                          const labelId = `${idPrefix}-label`;
                          const descriptionId = `${idPrefix}-description`;
                          return (
                            <FormControlLabel
                              control={
                                <Checkbox
                                  checked={selected.includes(permission.code)}
                                  onChange={(event) =>
                                    toggle(
                                      permission.code,
                                      event.target.checked,
                                    )
                                  }
                                  slotProps={{
                                    input: {
                                      'aria-describedby': descriptionId,
                                      'aria-labelledby': labelId,
                                    },
                                  }}
                                />
                              }
                              key={permission.code}
                              label={
                                <Box sx={{ py: 1 }}>
                                  <Typography
                                    component="span"
                                    id={labelId}
                                    sx={{
                                      display: 'block',
                                      fontFamily: monoFontFamily,
                                    }}
                                    variant="body2"
                                  >
                                    {permission.code}
                                  </Typography>
                                  <Typography
                                    color="text.secondary"
                                    id={descriptionId}
                                    sx={{ display: 'block' }}
                                    variant="caption"
                                  >
                                    {permission.description}
                                  </Typography>
                                </Box>
                              }
                              sx={{
                                alignItems: 'flex-start',
                                display: 'flex',
                                mr: 0,
                              }}
                            />
                          );
                        })}
                      </Box>
                    ),
                  )}
                </Box>
              </ProfileSection>
            );
          }}
        </form.Field>
        <Stack direction="row" spacing={3} sx={{ justifyContent: 'flex-end' }}>
          <Button component={Link} to={tokensPath} variant="outlined">
            {t('common.cancel')}
          </Button>
          <Button disabled={isCreating} type="submit" variant="contained">
            {t('profile.create')}
          </Button>
        </Stack>
      </Stack>
    </SettingsPage>
  );
};
