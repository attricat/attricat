import { Button, Chip, Paper, Stack, Typography } from '@mui/material';
import { useMutation } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { ErrorNotice } from './ExtensionErrorNotice';
import { grantExtension, revokeExtensionGrant } from './managementApi';

type GrantKind =
  'capability' | 'host_permission' | 'event_publish' | 'event_subscribe';

type Grant = { grant_kind: GrantKind; grant_id: string };

type ManifestPermissions = {
  permissions?: string[];
  optional_permissions?: string[];
  host_permissions?: { id: string }[];
  optional_host_permissions?: { id: string }[];
  event_contracts?: {
    exports?: { id: string }[];
    consumes?: { provider: string; contract: string }[];
  };
};

export const ExtensionPermissionsSection = ({
  enabled,
  extensionId,
  grants,
  manifest,
  onChanged,
}: {
  enabled: boolean;
  extensionId: string;
  grants: Grant[];
  manifest: unknown;
  onChanged: () => void;
}) => {
  const { t } = useTranslation();
  const grant = useMutation({
    mutationFn: ({ kind, id }: { kind: GrantKind; id: string }) =>
      grantExtension(extensionId, kind, id),
    onSuccess: onChanged,
  });
  const revoke = useMutation({
    mutationFn: ({ kind, id }: { kind: GrantKind; id: string }) =>
      revokeExtensionGrant(extensionId, kind, id),
    onSuccess: onChanged,
  });
  const value = manifest as ManifestPermissions;
  const requested: Array<readonly [GrantKind, string]> = [
    ...(value.permissions ?? []),
    ...(value.optional_permissions ?? []),
  ].map((id) => ['capability', id] as const);
  requested.push(
    ...[
      ...(value.host_permissions ?? []),
      ...(value.optional_host_permissions ?? []),
    ].map((item) => ['host_permission', item.id] as const),
    ...(value.event_contracts?.exports ?? []).map(
      (item) => ['event_publish', item.id] as const,
    ),
    ...(value.event_contracts?.consumes ?? []).map(
      (item) =>
        ['event_subscribe', `${item.provider}:${item.contract}`] as const,
    ),
  );

  return (
    <Paper sx={{ p: 2 }}>
      <Typography variant="h6">{t('extensions.permissions')}</Typography>
      <Stack spacing={1} sx={{ mt: 1 }}>
        <ErrorNotice error={grant.error ?? revoke.error} />
        {grants.map((item) => (
          <Stack
            direction="row"
            key={`${item.grant_kind}:${item.grant_id}`}
            spacing={1}
            sx={{ alignItems: 'center' }}
          >
            <Chip label={`${item.grant_kind}: ${item.grant_id}`} />
            <Button
              disabled={!enabled || revoke.isPending}
              onClick={() =>
                revoke.mutate({ kind: item.grant_kind, id: item.grant_id })
              }
            >
              {t('extensions.revoke')}
            </Button>
          </Stack>
        ))}
        {requested
          .filter(
            ([kind, id]) =>
              !grants.some(
                (item) => item.grant_kind === kind && item.grant_id === id,
              ),
          )
          .map(([kind, id]) => (
            <Stack direction="row" key={`${kind}:${id}`} spacing={1}>
              <Chip label={t('extensions.requestedGrant', { kind, id })} />
              <Button
                disabled={!enabled || grant.isPending}
                onClick={() => grant.mutate({ kind, id })}
              >
                {t('extensions.grant')}
              </Button>
            </Stack>
          ))}
      </Stack>
    </Paper>
  );
};
