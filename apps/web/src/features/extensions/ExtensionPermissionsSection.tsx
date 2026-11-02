import {
  Button,
  Chip,
  Paper,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
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
import { ActionsHeaderCell } from '../../components/ActionsHeaderCell';
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
  const pending = pendingGrants(manifest, grants);
  const permissionName = (kind: RequestedGrant['kind'], id: string) =>
    t('extensions.grantedPermission', {
      kind: t(grantKindLabelKey(kind)),
      id,
    });

  return (
    <Paper sx={{ p: 4 }}>
      <Typography component="h2" sx={{ mb: 1 }} variant="h6">
        {t('extensions.permissions')}
      </Typography>
      <ApiErrorAlert error={grant.error ?? revoke.error} sx={{ mb: 2 }} />
      {pending.length === 0 && grants.length === 0 ? (
        <Typography color="text.secondary">
          {t('extensions.noPermissions')}
        </Typography>
      ) : (
        <TableContainer>
          <Table>
            <TableHead>
              <TableRow>
                <TableCell>{t('extensions.permissionColumn')}</TableCell>
                <TableCell>{t('extensions.permissionKindColumn')}</TableCell>
                <TableCell>{t('extensions.permissionStatusColumn')}</TableCell>
                <ActionsHeaderCell />
              </TableRow>
            </TableHead>
            <TableBody>
              {pending.map((item) => (
                <TableRow key={grantKey(item.kind, item.id)}>
                  <TableCell sx={{ overflowWrap: 'anywhere' }}>
                    {item.id}
                  </TableCell>
                  <TableCell>{t(grantKindLabelKey(item.kind))}</TableCell>
                  <TableCell>
                    <Chip
                      color="warning"
                      label={t('extensions.permissionRequested')}
                      size="small"
                    />
                  </TableCell>
                  <TableCell align="right">
                    <Button
                      aria-label={t('extensions.grantNamed', {
                        permission: permissionName(item.kind, item.id),
                      })}
                      disabled={!enabled || grant.isPending}
                      onClick={() => grant.mutate(item)}
                    >
                      {t('extensions.grant')}
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
              {grants.map((item) => (
                <TableRow key={grantKey(item.grant_kind, item.grant_id)}>
                  <TableCell sx={{ overflowWrap: 'anywhere' }}>
                    {item.grant_id}
                  </TableCell>
                  <TableCell>{t(grantKindLabelKey(item.grant_kind))}</TableCell>
                  <TableCell>
                    <Chip
                      color="success"
                      label={t('extensions.permissionGranted')}
                      size="small"
                    />
                  </TableCell>
                  <TableCell align="right">
                    <Button
                      aria-label={t('extensions.revokeNamed', {
                        permission: permissionName(
                          item.grant_kind,
                          item.grant_id,
                        ),
                      })}
                      disabled={!enabled || revoke.isPending}
                      onClick={() =>
                        revoke.mutate({
                          kind: item.grant_kind,
                          id: item.grant_id,
                        })
                      }
                    >
                      {t('extensions.revoke')}
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </TableContainer>
      )}
    </Paper>
  );
};
