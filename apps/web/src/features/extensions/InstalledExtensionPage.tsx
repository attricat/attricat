import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { Stack } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { useCan } from '../auth/useCan';
import {
  extensionDetail,
  lifecycleExtension,
  removeExtension,
  upgradeExtension,
  type ExtensionLifecycleAction,
  type ExtensionReleaseReference,
} from './managementApi';
import { extensionManagementQueryKeys } from './managementQueryKeys';
import { invalidateExtensions } from './extensionPageUtils';
import { ExtensionConfigurationSection } from './ExtensionConfigurationSection';
import { ExtensionInstallationStatusSection } from './ExtensionInstallationStatusSection';
import { ExtensionLifecycleSection } from './ExtensionLifecycleSection';
import { ExtensionPermissionsSection } from './ExtensionPermissionsSection';
import { ExtensionUpgradeSection } from './ExtensionUpgradeSection';
import { ExtensionIcon } from '../../components/systemIcons';
import { ExtensionBudgetsSection } from '../extension-exports/ExtensionBudgetsSection';
import { ExtensionConnectionsSection } from '../extension-exports/ExtensionConnectionsSection';
import { ExtensionExportStatusSection } from '../extension-exports/ExtensionExportStatusSection';
import { manifestExportsSchema } from '../extension-exports/schemas';
import { capabilities } from '../auth/capabilities';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';

type InstallationAction =
  | { action: ExtensionLifecycleAction }
  | { action: 'remove' }
  | { action: 'upgrade'; release: ExtensionReleaseReference };

const runInstallationAction = (
  extensionId: string,
  input: InstallationAction,
): Promise<unknown> => {
  switch (input.action) {
    case 'remove':
      return removeExtension(extensionId);
    case 'upgrade':
      return upgradeExtension(extensionId, input.release);
    default:
      return lifecycleExtension(extensionId, input.action);
  }
};

export const InstalledExtensionPage = ({
  extensionId,
}: {
  extensionId: string;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const detail = useQuery({
    queryKey: extensionManagementQueryKeys.detail(extensionId),
    queryFn: () => extensionDetail(extensionId),
  });
  const canManage = useCan(capabilities.extensionsManage);
  const action = useMutation({
    mutationFn: (input: InstallationAction) =>
      runInstallationAction(extensionId, input).then(() => undefined),
    onSuccess: () => invalidateExtensions(client),
  });
  const installation = detail.data?.installation;
  const exportManifest = manifestExportsSchema.safeParse(
    installation?.manifest,
  );
  return (
    <PageContainer>
      <PageHeader
        icon={ExtensionIcon}
        title={installation?.extension_id ?? t('extensions.extensionFallback')}
        description={
          installation
            ? t('extensions.installationVersion', {
                version: installation.version,
                source: installation.source,
              })
            : t('extensions.loadingInstallation')
        }
        actions={<Link to="/manage/extensions">{t('extensions.back')}</Link>}
      />
      <ApiErrorAlert error={detail.error} sx={{ mb: 2 }} />
      <ApiErrorAlert error={action.error} sx={{ mb: 2 }} />
      {detail.data && installation && (
        <Stack spacing={3}>
          <ExtensionInstallationStatusSection
            busy={action.isPending}
            canManage={canManage}
            installation={installation}
            onLifecycleAction={(lifecycleAction) =>
              action.mutate({ action: lifecycleAction })
            }
            onRemove={() => action.mutate({ action: 'remove' })}
          />
          <ExtensionUpgradeSection
            busy={action.isPending}
            canManage={canManage}
            installation={installation}
            onUpgrade={(release) =>
              action.mutate({ action: 'upgrade', release })
            }
          />
          <ExtensionConfigurationSection
            canManage={canManage}
            extensionId={extensionId}
            installation={installation}
            key={installation.installed_release_id}
          />
          <ExtensionPermissionsSection
            enabled={canManage}
            extensionId={extensionId}
            grants={detail.data.grants}
            manifest={installation.manifest}
            onChanged={() => invalidateExtensions(client)}
          />
          {exportManifest.success && (
            <>
              <ExtensionExportStatusSection
                canManage={canManage}
                extensionId={extensionId}
                manifest={exportManifest.data}
              />
              <ExtensionConnectionsSection
                canManage={canManage}
                extensionId={extensionId}
                manifest={exportManifest.data}
              />
            </>
          )}
          <ExtensionBudgetsSection
            canManage={canManage}
            extensionId={extensionId}
          />
          <ExtensionLifecycleSection lifecycle={detail.data.lifecycle} />
        </Stack>
      )}
    </PageContainer>
  );
};
