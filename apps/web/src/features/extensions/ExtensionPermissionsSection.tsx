import { Button, Chip, Paper, Stack, Typography } from '@mui/material';
import { useMutation } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { ErrorNotice } from './ExtensionErrorNotice';
import {
  grantKey,
  pendingGrants,
  type ExtensionGrant,
  type RequestedGrant,
} from './extensionGrants';
import { grantKindLabelKey } from './extensionPageUtils';
import { grantExtension, revokeExtensionGrant } from './managementApi';

type ExtensionPermissionsSectionProps = {
  enabled: boolean;
  extensionId: string;
  grants: ExtensionGrant[];
  manifest: unknown;
  onChanged: () => void;
};

export const ExtensionPermissionsSection = ({
  enabled,
  extensionId,
  grants,
  manifest,
  onChanged,
}: ExtensionPermissionsSectionProps) => {
  const { t } = useTranslation();
  const grant = useMutation({
    mutationFn: ({ kind, id }: RequestedGrant) =>
      grantExtension(extensionId, kind, id),
    onSuccess: onChanged,
  });
  const revoke = useMutation({
    mutationFn: ({ kind, id }: RequestedGrant) =>
      revokeExtensionGrant(extensionId, kind, id),
    onSuccess: onChanged,
  });

  return (
    <Paper sx={{ p: 2 }}>
      <Typography variant="h6">{t('extensions.permissions')}</Typography>
      <Stack spacing={1} sx={{ mt: 1 }}>
        <ErrorNotice error={grant.error ?? revoke.error} />
        {grants.map((item) => (
          <Stack
            direction="row"
            key={grantKey(item.grant_kind, item.grant_id)}
            spacing={1}
            sx={{ alignItems: 'center' }}
          >
            <Chip
              label={t('extensions.grantedPermission', {
                kind: t(grantKindLabelKey(item.grant_kind)),
                id: item.grant_id,
              })}
            />
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
        {pendingGrants(manifest, grants).map((item) => (
          <Stack direction="row" key={grantKey(item.kind, item.id)} spacing={1}>
            <Chip
              label={t('extensions.requestedGrant', {
                kind: t(grantKindLabelKey(item.kind)),
                id: item.id,
              })}
            />
            <Button
              disabled={!enabled || grant.isPending}
              onClick={() => grant.mutate(item)}
            >
              {t('extensions.grant')}
            </Button>
          </Stack>
        ))}
      </Stack>
    </Paper>
  );
};
