import { Alert } from '@mui/material';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { ConfirmDialog } from '../../components/ConfirmDialog';
import { LoadingState } from '../../components/LoadingState';
import { invalidateExtensions, releaseTagVersion } from './extensionPageUtils';
import { registryDetails, upgradeExtension } from './managementApi';
import { extensionManagementQueryKeys } from './managementQueryKeys';

/** A registry upgrade the person has asked for and has yet to confirm. */
export type PendingExtensionUpgrade = {
  extensionId: string;
  extensionName: string;
  installedVersion: string;
  owner: string;
  repository: string;
  /** The reviewed version to upgrade to, without a tag's `v`. */
  version: string;
};

/**
 * Asks before upgrading an installed extension to one of its registry's
 * reviewed releases, then upgrades it. Every registry upgrade goes through it.
 */
export const ExtensionUpgradeDialog = ({
  onClose,
  upgrade,
}: {
  onClose: () => void;
  upgrade: PendingExtensionUpgrade | null;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const owner = upgrade?.owner ?? '';
  const repository = upgrade?.repository ?? '';
  // The update list names only a version; the upgrade needs its GitHub release.
  const details = useQuery({
    queryKey: extensionManagementQueryKeys.registry(owner, repository),
    queryFn: () => registryDetails(owner, repository),
    enabled: upgrade !== null,
  });
  const release = details.data?.releases.find(
    (candidate) =>
      upgrade !== null &&
      releaseTagVersion(candidate.tag_name) === upgrade.version,
  );
  const action = useMutation({
    mutationFn: (pending: PendingExtensionUpgrade & { releaseId: number }) =>
      upgradeExtension(pending.extensionId, {
        owner: pending.owner,
        repository: pending.repository,
        release_id: pending.releaseId,
      }),
    onSuccess: () => invalidateExtensions(client),
  });
  const close = () => {
    action.reset();
    onClose();
  };
  return (
    <ConfirmDialog
      confirmDisabled={!release}
      confirmLabel={t('extensions.upgrade')}
      error={action.error ?? details.error}
      isPending={action.isPending}
      onClose={close}
      onConfirm={() => {
        if (upgrade && release)
          action.mutate(
            { ...upgrade, releaseId: release.release_id },
            { onSuccess: close },
          );
      }}
      open={upgrade !== null}
      title={t('extensions.upgradeQuestion', { name: upgrade?.extensionName })}
    >
      {t('extensions.upgradeVersions', {
        from: upgrade?.installedVersion,
        to: upgrade?.version,
      })}{' '}
      {t('extensions.upgradeEffect')}
      {details.isFetching && !release && (
        <LoadingState sx={{ mt: 2 }}>
          {t('extensions.loadingRelease')}
        </LoadingState>
      )}
      {details.isSuccess && !details.isFetching && !release && (
        <Alert severity="warning" sx={{ mt: 2 }}>
          {t('extensions.upgradeReleaseMissing', { version: upgrade?.version })}
        </Alert>
      )}
    </ConfirmDialog>
  );
};
