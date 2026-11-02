import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, Box, Chip, Stack, Tabs, Typography } from '@mui/material';
import {
  ArrowLeftIcon,
  HistoryIcon,
  InfoIcon,
  SlidersHorizontalIcon,
  type LucideIcon,
} from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { ConfirmDialog } from '../../components/ConfirmDialog';
import { compactIconSize } from '../../components/iconSizes';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { RouterButton, RouterTab } from '../../components/RouterLink';
import { useTabAccessibility } from '../../components/useTabAccessibility';
import { useCan } from '../auth/useCan';
import {
  extensionDetail,
  lifecycleExtension,
  removeExtension,
  upgradeExtension,
  upgradeExtensionFromArchive,
  type ExtensionDetail,
  type ExtensionLifecycleAction,
  type ExtensionReleaseReference,
} from './managementApi';
import {
  installedExtensionTabs,
  type InstalledExtensionTab,
} from './constants';
import { pendingGrants } from './extensionGrants';
import { extensionManagementQueryKeys } from './managementQueryKeys';
import {
  installationStateColor,
  installationStateLabelKey,
  installedExtensionDescription,
  installedExtensionName,
  invalidateExtensions,
} from './extensionPageUtils';
import { ExtensionConfigurationSection } from './ExtensionConfigurationSection';
import { ExtensionDetailsSection } from './ExtensionDetailsSection';
import { ExtensionInstallationActions } from './ExtensionInstallationActions';
import { ExtensionLifecycleSection } from './ExtensionLifecycleSection';
import { ExtensionPermissionsSection } from './ExtensionPermissionsSection';
import { ExtensionUpgradeSection } from './ExtensionUpgradeSection';
import {
  ExportIcon,
  ExtensionConnectionIcon,
  ExtensionIcon,
  PermissionIcon,
} from '../../components/systemIcons';
import { ExtensionBudgetsSection } from '../extension-exports/ExtensionBudgetsSection';
import { ExtensionConnectionsSection } from '../extension-exports/ExtensionConnectionsSection';
import { ExtensionExportStatusSection } from '../extension-exports/ExtensionExportStatusSection';
import { manifestExportsSchema } from '../extension-exports/schemas';
import { capabilities } from '../auth/capabilities';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';

const tabIcons = {
  [installedExtensionTabs.overview]: InfoIcon,
  [installedExtensionTabs.configuration]: SlidersHorizontalIcon,
  [installedExtensionTabs.permissions]: PermissionIcon,
  [installedExtensionTabs.exports]: ExportIcon,
  [installedExtensionTabs.connections]: ExtensionConnectionIcon,
  [installedExtensionTabs.lifecycle]: HistoryIcon,
} satisfies Record<InstalledExtensionTab, LucideIcon>;

type InstallationAction =
  | { action: ExtensionLifecycleAction }
  | { action: 'remove' }
  | { action: 'upgrade'; release: ExtensionReleaseReference }
  | { action: 'upgradeArchive'; archive: File };

const runInstallationAction = (
  extensionId: string,
  input: InstallationAction,
): Promise<unknown> => {
  switch (input.action) {
    case 'remove':
      return removeExtension(extensionId);
    case 'upgrade':
      return upgradeExtension(extensionId, input.release);
    case 'upgradeArchive':
      return upgradeExtensionFromArchive(extensionId, input.archive);
    default:
      return lifecycleExtension(extensionId, input.action);
  }
};

/**
 * One installation's sections. Every panel stays mounted while hidden so
 * switching tabs keeps unsaved configuration, budgets and export filters.
 */
const InstalledExtensionTabs = ({
  activeTab,
  busy,
  canManage,
  detail,
  extensionId,
  onChanged,
  onUpgrade,
}: {
  activeTab: InstalledExtensionTab;
  busy: boolean;
  canManage: boolean;
  detail: ExtensionDetail;
  extensionId: string;
  onChanged: () => void;
  onUpgrade: (release: ExtensionReleaseReference) => void;
}) => {
  const { t } = useTranslation();
  const tabId = useTabAccessibility();
  // The export status lists records across the workspace, so it needs
  // workspace-wide read access.
  const canReadRecords = useCan(capabilities.recordsRead);
  const { grants, installation, lifecycle } = detail;
  const pending = pendingGrants(installation.manifest, grants).length;
  const manifest = manifestExportsSchema.safeParse(installation.manifest).data;
  const panels: { content: ReactNode; key: InstalledExtensionTab }[] = [
    {
      key: installedExtensionTabs.overview,
      content: (
        <Stack spacing={4}>
          {installation.manifest_unreadable && (
            <Alert severity="error">{t('extensions.manifestUnreadable')}</Alert>
          )}
          {pending > 0 && (
            <Alert
              action={
                <RouterButton
                  color="inherit"
                  params={{ extensionId }}
                  search={{ tab: installedExtensionTabs.permissions }}
                  to="/manage/extensions/$extensionId"
                >
                  {t('extensions.reviewPermissions')}
                </RouterButton>
              }
              severity="warning"
            >
              {t('extensions.pendingPermissions', { count: pending })}
            </Alert>
          )}
          <ExtensionDetailsSection installation={installation} />
          <ExtensionUpgradeSection
            busy={busy}
            canManage={canManage}
            installation={installation}
            onUpgrade={onUpgrade}
          />
        </Stack>
      ),
    },
    {
      key: installedExtensionTabs.configuration,
      content: (
        <ExtensionConfigurationSection
          canManage={canManage}
          extensionId={extensionId}
          installation={installation}
          key={installation.installed_release_id}
        />
      ),
    },
    {
      key: installedExtensionTabs.permissions,
      content: (
        <Stack spacing={4}>
          <ExtensionPermissionsSection
            enabled={canManage}
            extensionId={extensionId}
            grants={grants}
            manifest={installation.manifest}
            onChanged={onChanged}
          />
          <ExtensionBudgetsSection
            canManage={canManage}
            extensionId={extensionId}
          />
        </Stack>
      ),
    },
    ...(manifest && canReadRecords && manifest.server?.exports.length
      ? [
          {
            key: installedExtensionTabs.exports,
            content: (
              <ExtensionExportStatusSection
                canManage={canManage}
                extensionId={extensionId}
                manifest={manifest}
              />
            ),
          },
        ]
      : []),
    ...(manifest && manifest.server?.connections.length
      ? [
          {
            key: installedExtensionTabs.connections,
            content: (
              <ExtensionConnectionsSection
                canManage={canManage}
                extensionId={extensionId}
                manifest={manifest}
              />
            ),
          },
        ]
      : []),
    {
      key: installedExtensionTabs.lifecycle,
      content: <ExtensionLifecycleSection lifecycle={lifecycle} />,
    },
  ];
  // A link can name a tab this extension does not have.
  const shownTab = panels.some(({ key }) => key === activeTab)
    ? activeTab
    : installedExtensionTabs.overview;
  return (
    <>
      <Tabs
        allowScrollButtonsMobile
        aria-label={t('extensions.tabsLabel')}
        scrollButtons="auto"
        sx={{ mb: 4, mt: 3 }}
        value={shownTab}
        variant="scrollable"
      >
        {panels.map(({ key }, index) => {
          const Icon = tabIcons[key];
          return (
            <RouterTab
              {...tabId.tab(index)}
              icon={<Icon aria-hidden size={compactIconSize} />}
              key={key}
              label={t(`extensions.tabs.${key}`)}
              params={{ extensionId }}
              search={{ tab: key }}
              to="/manage/extensions/$extensionId"
              value={key}
            />
          );
        })}
      </Tabs>
      {panels.map(({ content, key }, index) => (
        <Box {...tabId.panel(index)} hidden={key !== shownTab} key={key}>
          {content}
        </Box>
      ))}
    </>
  );
};

export const InstalledExtensionPage = ({
  extensionId,
  tab = installedExtensionTabs.overview,
}: {
  extensionId: string;
  tab?: InstalledExtensionTab;
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
  // Waiting for the person to confirm upgrading from this archive.
  const [archive, setArchive] = useState<File | null>(null);
  const closeArchiveDialog = () => {
    setArchive(null);
    action.reset();
  };
  const installation = detail.data?.installation;
  return (
    <PageContainer>
      <RouterButton
        startIcon={<ArrowLeftIcon />}
        sx={{ mb: 2 }}
        to="/manage/extensions"
      >
        {t('extensions.back')}
      </RouterButton>
      <PageHeader
        actions={
          canManage &&
          installation && (
            <ExtensionInstallationActions
              busy={action.isPending}
              installation={installation}
              onArchiveChosen={(chosen) => {
                action.reset();
                setArchive(chosen);
              }}
              onLifecycleAction={(lifecycleAction) =>
                action.mutate({ action: lifecycleAction })
              }
              onRemove={() => action.mutate({ action: 'remove' })}
            />
          )
        }
        description={
          installation
            ? installedExtensionDescription(installation)
            : detail.isPending
              ? t('extensions.loadingInstallation')
              : undefined
        }
        icon={ExtensionIcon}
        title={
          installation
            ? installedExtensionName(installation)
            : t('extensions.extensionFallback')
        }
      />
      {installation && (
        <Stack direction="row" spacing={1} sx={{ alignItems: 'center', mt: 1 }}>
          <Chip
            color={installationStateColor(installation.state)}
            label={t(installationStateLabelKey(installation.state))}
            size="small"
          />
          <Typography color="text.secondary" variant="body2">
            {t('extensions.versionLabel', { version: installation.version })}
          </Typography>
        </Stack>
      )}
      <ApiErrorAlert error={detail.error} sx={{ mt: 2 }} />
      <ApiErrorAlert error={archive ? null : action.error} sx={{ mt: 2 }} />
      {detail.data && (
        <InstalledExtensionTabs
          activeTab={tab}
          busy={action.isPending}
          canManage={canManage}
          detail={detail.data}
          extensionId={extensionId}
          onChanged={() => invalidateExtensions(client)}
          onUpgrade={(release) => action.mutate({ action: 'upgrade', release })}
        />
      )}
      <ConfirmDialog
        confirmLabel={t('extensions.upgrade')}
        error={action.error}
        isPending={action.isPending}
        onClose={closeArchiveDialog}
        onConfirm={() => {
          if (archive)
            action.mutate(
              { action: 'upgradeArchive', archive },
              { onSuccess: () => setArchive(null) },
            );
        }}
        open={archive !== null}
        title={t('extensions.upgradeFromArchive')}
      >
        {t('extensions.upgradeArchiveQuestion', { name: archive?.name })}{' '}
        {t('extensions.upgradeEffect')}
      </ConfirmDialog>
    </PageContainer>
  );
};
