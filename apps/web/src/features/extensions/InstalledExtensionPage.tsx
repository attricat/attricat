import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Alert, Box, Stack, Tabs, Typography } from '@mui/material';
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
import {
  RouterButton,
  RouterLink,
  RouterTab,
} from '../../components/RouterLink';
import { useTabAccessibility } from '../../components/useTabAccessibility';
import { useCan } from '../auth/useCan';
import {
  extensionDetail,
  lifecycleExtension,
  removeExtension,
  upgradeExtensionFromArchive,
  type ExtensionDetail,
  type ExtensionLifecycleAction,
} from './managementApi';
import {
  installedExtensionTabs,
  secretsReadCapability,
  type InstalledExtensionTab,
} from './constants';
import { pendingGrants, requestsCapability } from './extensionGrants';
import { extensionSecretsOptions } from './queryOptions';
import { ExtensionSecretsSection } from './ExtensionSecretsSection';
import { extensionManagementQueryKeys } from './managementQueryKeys';
import {
  installedExtensionDescription,
  installedExtensionName,
  invalidateExtensions,
  marketplaceRepository,
} from './extensionPageUtils';
import { ExtensionConfigurationSection } from './ExtensionConfigurationSection';
import { ExtensionDetailsSection } from './ExtensionDetailsSection';
import { ExtensionInstallationActions } from './ExtensionInstallationActions';
import { ExtensionLifecycleSection } from './ExtensionLifecycleSection';
import { ExtensionPermissionsSection } from './ExtensionPermissionsSection';
import { ExtensionUpdateChip } from './ExtensionUpdateChip';
import {
  ExtensionUpgradeDialog,
  type PendingExtensionUpgrade,
} from './ExtensionUpgradeDialog';
import { ExtensionUpgradeSection } from './ExtensionUpgradeSection';
import { useExtensionUpdates } from './useExtensionUpdates';
import {
  ExportIcon,
  ExtensionConnectionIcon,
  ExtensionIcon,
  ExtensionFailureIcon,
  ExtensionSecretIcon,
  ExtensionWebhookIcon,
  PermissionIcon,
} from '../../components/systemIcons';
import { ExtensionBudgetsSection } from '../extension-exports/ExtensionBudgetsSection';
import { ExtensionConnectionsSection } from '../extension-exports/ExtensionConnectionsSection';
import { ExtensionExportStatusSection } from '../extension-exports/ExtensionExportStatusSection';
import { manifestExportsSchema } from '../extension-exports/schemas';
import { ExtensionWebhooksSection } from '../extension-webhooks/ExtensionWebhooksSection';
import { ExtensionFailuresSection } from '../extension-failures/ExtensionFailuresSection';
import { hasBackgroundWork } from '../extension-failures/schemas';
import { manifestWebhooks } from '../extension-webhooks/schemas';
import { capabilities } from '../auth/capabilities';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { InstallationStateChip } from './InstallationStateChip';

const tabIcons = {
  [installedExtensionTabs.overview]: InfoIcon,
  [installedExtensionTabs.configuration]: SlidersHorizontalIcon,
  [installedExtensionTabs.permissions]: PermissionIcon,
  [installedExtensionTabs.exports]: ExportIcon,
  [installedExtensionTabs.connections]: ExtensionConnectionIcon,
  [installedExtensionTabs.webhooks]: ExtensionWebhookIcon,
  [installedExtensionTabs.secrets]: ExtensionSecretIcon,
  [installedExtensionTabs.failures]: ExtensionFailureIcon,
  [installedExtensionTabs.lifecycle]: HistoryIcon,
} satisfies Record<InstalledExtensionTab, LucideIcon>;

type InstallationAction =
  | { action: ExtensionLifecycleAction }
  | { action: 'remove' }
  | { action: 'upgradeArchive'; archive: File };

const runInstallationAction = (
  extensionId: string,
  input: InstallationAction,
): Promise<unknown> => {
  switch (input.action) {
    case 'remove':
      return removeExtension(extensionId);
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
  onUpgrade: (version: string) => void;
}) => {
  const { t } = useTranslation();
  const tabId = useTabAccessibility();
  // The export status lists records across the workspace, so it needs
  // workspace-wide read access.
  const canReadRecords = useCan(capabilities.recordsRead);
  const { grants, installation } = detail;
  const pending = pendingGrants(installation.manifest, grants).length;
  const manifest = manifestExportsSchema.safeParse(installation.manifest).data;
  const webhooks = manifestWebhooks(installation.manifest);
  // Listing secrets, even by name, needs extension management.
  const showSecrets =
    canManage &&
    requestsCapability(installation.manifest, secretsReadCapability);
  const secrets = useQuery({
    ...extensionSecretsOptions(extensionId),
    enabled: showSecrets,
  });
  const missingSecrets =
    secrets.data?.filter((secret) => secret.required && !secret.set).length ??
    0;
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
          {missingSecrets > 0 && (
            <Alert
              action={
                <RouterButton
                  color="inherit"
                  params={{ extensionId }}
                  search={{ tab: installedExtensionTabs.secrets }}
                  to="/manage/extensions/$extensionId"
                >
                  {t('extensions.reviewSecrets')}
                </RouterButton>
              }
              severity="warning"
            >
              {t('extensions.missingSecrets', { count: missingSecrets })}
            </Alert>
          )}
          <ExtensionUpgradeSection
            busy={busy}
            canManage={canManage}
            installation={installation}
            onUpgrade={onUpgrade}
          />
          <ExtensionDetailsSection installation={installation} />
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
    ...(webhooks.length
      ? [
          {
            key: installedExtensionTabs.webhooks,
            content: (
              <ExtensionWebhooksSection
                canManage={canManage}
                extensionId={extensionId}
                webhooks={webhooks}
              />
            ),
          },
        ]
      : []),
    ...(showSecrets
      ? [
          {
            key: installedExtensionTabs.secrets,
            content: (
              <ExtensionSecretsSection
                canManage={canManage}
                extensionId={extensionId}
              />
            ),
          },
        ]
      : []),
    // Failures name records and carry the extension's error text, so they
    // need workspace-wide read access like the export status.
    ...(canReadRecords && hasBackgroundWork(installation.manifest)
      ? [
          {
            key: installedExtensionTabs.failures,
            content: <ExtensionFailuresSection extensionId={extensionId} />,
          },
        ]
      : []),
    {
      key: installedExtensionTabs.lifecycle,
      content: <ExtensionLifecycleSection extensionId={extensionId} />,
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
  const navigate = useNavigate();
  const detail = useQuery({
    queryKey: extensionManagementQueryKeys.detail(extensionId),
    queryFn: () => extensionDetail(extensionId),
  });
  const canManage = useCan(capabilities.extensionsManage);
  const action = useMutation({
    mutationFn: (input: InstallationAction) =>
      runInstallationAction(extensionId, input).then(() => undefined),
    onSuccess: async (_, { action }) => {
      // Leave first: the still-mounted detail query would refetch into a 404.
      if (action === 'remove') {
        await navigate({ to: '/manage/extensions/installed' });
        client.removeQueries({
          queryKey: extensionManagementQueryKeys.detail(extensionId),
        });
      }
      await invalidateExtensions(client);
    },
  });
  // Waiting for the person to confirm upgrading from this archive.
  const [archive, setArchive] = useState<File | null>(null);
  // Waiting for the person to confirm upgrading to this registry release.
  const [upgrade, setUpgrade] = useState<PendingExtensionUpgrade | null>(null);
  const updates = useExtensionUpdates();
  const [removeOpen, setRemoveOpen] = useState(false);
  const closeArchiveDialog = () => {
    setArchive(null);
    action.reset();
  };
  const installation = detail.data?.installation;
  const marketplace = installation
    ? marketplaceRepository(installation.source)
    : undefined;
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
              onRemove={() => {
                action.reset();
                setRemoveOpen(true);
              }}
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
          <InstallationStateChip state={installation.state} />
          <ExtensionUpdateChip
            update={updates.find(
              (update) => update.extension_id === installation.extension_id,
            )}
          />
          <Typography color="text.secondary" variant="body2">
            {t('extensions.versionLabel', { version: installation.version })}
          </Typography>
          {marketplace && (
            <RouterLink
              params={marketplace}
              to="/manage/extensions/$owner/$repository"
              variant="body2"
            >
              {t('extensions.viewInMarketplace')}
            </RouterLink>
          )}
        </Stack>
      )}
      <ApiErrorAlert error={detail.error} sx={{ mt: 2 }} />
      <ApiErrorAlert
        error={archive || removeOpen ? null : action.error}
        sx={{ mt: 2 }}
      />
      {detail.data && (
        <InstalledExtensionTabs
          activeTab={tab}
          busy={action.isPending}
          canManage={canManage}
          detail={detail.data}
          extensionId={extensionId}
          onChanged={() => invalidateExtensions(client)}
          onUpgrade={(version) => {
            if (marketplace)
              setUpgrade({
                ...marketplace,
                extensionId,
                extensionName: installedExtensionName(detail.data.installation),
                installedVersion: detail.data.installation.version,
                version,
              });
          }}
        />
      )}
      <ExtensionUpgradeDialog
        onClose={() => setUpgrade(null)}
        upgrade={upgrade}
      />
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
      <ConfirmDialog
        confirmLabel={t('extensions.remove')}
        destructive
        error={action.error}
        isPending={action.isPending}
        onClose={() => {
          setRemoveOpen(false);
          action.reset();
        }}
        onConfirm={() => action.mutate({ action: 'remove' })}
        open={removeOpen}
        title={t('extensions.removeQuestion', {
          name: installation && installedExtensionName(installation),
        })}
      >
        {t('extensions.removeEffect')}
      </ConfirmDialog>
    </PageContainer>
  );
};
