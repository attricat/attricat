import { Button, Chip, Paper, Stack, Typography } from '@mui/material';
import { useMutation } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  grantKey,
  pendingGrants,
  type ExtensionGrant,
  type RequestedGrant,
} from './extensionGrants';
import { grantKindLabelKey } from './extensionPageUtils';
import { grantExtension, revokeExtensionGrant } from './managementApi';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';

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
        <ApiErrorAlert error={grant.error ?? revoke.error} sx={{ mb: 2 }} />
        {grants.map((item) => {
          const permission = t('extensions.grantedPermission', {
            kind: t(grantKindLabelKey(item.grant_kind)),
            id: item.grant_id,
          });
          return (
            <Stack
              direction="row"
              key={grantKey(item.grant_kind, item.grant_id)}
              spacing={1}
              sx={{ alignItems: 'center' }}
            >
              <Chip label={permission} />
              <Button
                aria-label={t('extensions.revokeNamed', { permission })}
                disabled={!enabled || revoke.isPending}
                onClick={() =>
                  revoke.mutate({ kind: item.grant_kind, id: item.grant_id })
                }
              >
                {t('extensions.revoke')}
              </Button>
            </Stack>
          );
        })}
        {pendingGrants(manifest, grants).map((item) => {
          const kind = t(grantKindLabelKey(item.kind));
          const permission = t('extensions.grantedPermission', {
            kind,
            id: item.id,
          });
          return (
            <Stack
              direction="row"
              key={grantKey(item.kind, item.id)}
              spacing={1}
            >
              <Chip
                label={t('extensions.requestedGrant', { kind, id: item.id })}
              />
              <Button
                aria-label={t('extensions.grantNamed', { permission })}
                disabled={!enabled || grant.isPending}
                onClick={() => grant.mutate(item)}
              >
                {t('extensions.grant')}
              </Button>
            </Stack>
          );
        })}
      </Stack>
    </Paper>
  );
};
