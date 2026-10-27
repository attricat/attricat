import {
  blueprintRevisionOptions,
  blueprintRevisionListOptions,
} from './queryOptions';
import { revisionStatuses } from '../../api/revisionStatus';
import { useQuery } from '@tanstack/react-query';
import { Alert, Box, Tab, Tabs } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  CircleArrowUpIcon,
  GitCompareArrowsIcon,
  HistoryIcon,
  InfoIcon,
} from 'lucide-react';
import { compactIconSize } from '../../components/iconSizes';
import { useResourcePageTitle } from '../../app/useResourcePageTitle';
import { PageContainer } from '../../components/PageContainer';
import { useTabAccessibility } from '../../components/useTabAccessibility';
import { ExtensionOutlet } from '../extensions/ExtensionOutlet';
import { BlueprintDetailHeader } from './BlueprintDetailHeader';
import { BlueprintRevisionComparison } from './BlueprintRevisionComparison';
import { BlueprintVersionMetadata } from './BlueprintVersionMetadata';
import {
  archiveRemovalDisposition,
  blueprintDetailTabs,
  type BlueprintDetailTab,
  blueprintDetailTabNames,
  type BlueprintDetailTabName,
} from './constants';
import { MigrationBatchStatus } from './MigrationBatchStatus';
import { PublishBlueprintDialog } from './PublishBlueprintDialog';
import { PublishBlueprintRecordsDialog } from './PublishBlueprintRecordsDialog';
import { RevisionHistory } from './RevisionHistory';
import { SafeMigrationDialog } from './SafeMigrationDialog';
import { useBlueprintDetailActions } from './useBlueprintDetailActions';
import { useBlueprintRevisionComparison } from './useBlueprintRevisionComparison';
import { useSafeBlueprintMigration } from './useSafeBlueprintMigration';
import { lexiconText } from '../lexicon/lexicon';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import {
  extensionOutlets,
  supportedOutletContextVersion,
} from '../extensions/constants';
import { LoadingState } from '../../components/LoadingState';

type BlueprintDetailDialog = 'publish' | 'publishRecords' | 'safeMigration';

export const BlueprintDetailPage = ({
  blueprintId,
  initialTab = blueprintDetailTabNames.metadata,
}: {
  blueprintId: string;
  /** The tab shown first, such as Migrations when linked from progress. */
  initialTab?: BlueprintDetailTabName;
}) => {
  const { t } = useTranslation();
  const tabId = useTabAccessibility();
  const [pageTab, setPageTab] = useState<BlueprintDetailTab>(
    blueprintDetailTabs[initialTab],
  );
  const [openDialog, setOpenDialog] = useState<BlueprintDetailDialog | null>(
    null,
  );
  const closeDialog = () => setOpenDialog(null);
  const { publish, publishRecords, safeMigration } = useBlueprintDetailActions(
    blueprintId,
    {
      onRecordsPublished: closeDialog,
      onMigrationStarted: () => {
        closeDialog();
        setPageTab(blueprintDetailTabs.migrations);
      },
      onPublished: closeDialog,
    },
  );
  // Each dialog shows its own failure, so a reopened dialog starts clean.
  const openActionDialog = (dialog: BlueprintDetailDialog) => {
    ({ publish, publishRecords, safeMigration })[dialog].reset();
    setOpenDialog(dialog);
  };
  const revisions = useQuery(blueprintRevisionListOptions(blueprintId));
  const revisionItems = revisions.data ?? [];
  const comparison = useBlueprintRevisionComparison(blueprintId, revisionItems);
  const migration = useSafeBlueprintMigration(blueprintId, revisionItems);
  const blueprint = revisionItems[0];
  // Metadata describes the version records use: the latest published one,
  // or the latest draft before anything is published.
  const metadataVersion = (
    revisionItems.find(
      (revision) => revision.status === revisionStatuses.published,
    ) ?? blueprint
  )?.version;
  const metadata = useQuery(
    blueprintRevisionOptions(blueprintId, metadataVersion),
  );
  const metadataRevision = metadata.data;
  useResourcePageTitle(
    blueprint && lexiconText(blueprint.name),
    t('navigation.blueprints'),
  );

  return (
    <PageContainer>
      {revisions.isPending && (
        <LoadingState>{t('blueprints.loadingBlueprint')}</LoadingState>
      )}
      <ApiErrorAlert error={revisions.error} />
      {blueprint && (
        <>
          <BlueprintDetailHeader
            blueprint={blueprint}
            canStartSafeMigration={migration.canStart}
            migrationActive={migration.currentVersionMigrationActive}
            migrationStatusKnown={migration.migrationBatches.isSuccess}
            onMigrate={() => openActionDialog('safeMigration')}
            onPublish={() => openActionDialog('publish')}
            onPublishRecords={() => openActionDialog('publishRecords')}
          />
          {migration.wasEvaluated && !migration.canStart && (
            <Alert severity="info" sx={{ mt: 2 }}>
              {t('blueprints.automaticMigrationUnavailable')}
            </Alert>
          )}
          <Box component="aside" sx={{ mt: 3 }}>
            <ExtensionOutlet
              context={{
                context_version: supportedOutletContextVersion,
                blueprint_id: blueprint.id,
                blueprint_version: blueprint.version,
              }}
              outlet={extensionOutlets.blueprintPanel}
            />
          </Box>
          <Tabs
            allowScrollButtonsMobile
            onChange={(_, value: BlueprintDetailTab) => setPageTab(value)}
            scrollButtons="auto"
            sx={{ mt: 3 }}
            value={pageTab}
            variant="scrollable"
          >
            <Tab
              {...tabId.tab(blueprintDetailTabs.metadata)}
              icon={<InfoIcon aria-hidden size={compactIconSize} />}
              label={t('blueprints.versionMetadata', {
                version: metadataVersion,
              })}
            />
            <Tab
              {...tabId.tab(blueprintDetailTabs.revisionHistory)}
              icon={<HistoryIcon aria-hidden size={compactIconSize} />}
              label={t('blueprints.revisionHistory')}
            />
            <Tab
              {...tabId.tab(blueprintDetailTabs.compareDefinitions)}
              icon={<GitCompareArrowsIcon aria-hidden size={compactIconSize} />}
              label={t('blueprints.compareDefinitions')}
            />
            <Tab
              {...tabId.tab(blueprintDetailTabs.migrations)}
              icon={<CircleArrowUpIcon aria-hidden size={compactIconSize} />}
              label={t('blueprints.migrations')}
            />
          </Tabs>
          {pageTab === blueprintDetailTabs.metadata && (
            <Box {...tabId.panel(blueprintDetailTabs.metadata)}>
              {metadata.isPending && (
                <LoadingState>{t('blueprints.loadingBlueprint')}</LoadingState>
              )}
              <ApiErrorAlert error={metadata.error} />
              {metadataRevision && (
                <>
                  <BlueprintVersionMetadata blueprint={metadataRevision} />
                  <Box component="aside" sx={{ mt: 3 }}>
                    <ExtensionOutlet
                      context={{
                        context_version: supportedOutletContextVersion,
                        blueprint_id: metadataRevision.blueprint.id,
                        blueprint_version: metadataRevision.blueprint.version,
                      }}
                      outlet={extensionOutlets.blueprintDetailPanel}
                    />
                  </Box>
                </>
              )}
            </Box>
          )}
          {pageTab === blueprintDetailTabs.revisionHistory && (
            <Box {...tabId.panel(blueprintDetailTabs.revisionHistory)}>
              <RevisionHistory
                blueprintId={blueprintId}
                revisions={revisionItems}
              />
            </Box>
          )}
          {pageTab === blueprintDetailTabs.compareDefinitions && (
            <BlueprintRevisionComparison
              {...tabId.panel(blueprintDetailTabs.compareDefinitions)}
              comparison={comparison}
              revisions={revisionItems}
            />
          )}
          {pageTab === blueprintDetailTabs.migrations && (
            <Box {...tabId.panel(blueprintDetailTabs.migrations)}>
              <MigrationBatchStatus batches={migration.migrationBatches} />
            </Box>
          )}
          <PublishBlueprintRecordsDialog
            blueprint={blueprint}
            error={publishRecords.error}
            isPending={publishRecords.isPending}
            onClose={closeDialog}
            onConfirm={(contextId) =>
              publishRecords.mutate({ version: blueprint.version, contextId })
            }
            open={openDialog === 'publishRecords'}
          />
          <SafeMigrationDialog
            error={safeMigration.error}
            impact={migration.impact}
            isPending={safeMigration.isPending}
            onClose={closeDialog}
            onConfirm={() =>
              migration.targetVersion !== undefined &&
              safeMigration.mutate({
                version: migration.targetVersion,
                removalDisposition: migration.impact
                  ?.requires_removal_disposition
                  ? archiveRemovalDisposition
                  : undefined,
              })
            }
            open={openDialog === 'safeMigration'}
            sourceVersion={migration.sourceVersion}
            targetVersion={migration.targetVersion}
          />
          <PublishBlueprintDialog
            blueprint={blueprint}
            error={publish.error}
            isPending={publish.isPending}
            onClose={closeDialog}
            onConfirm={() => publish.mutate(blueprint.version)}
            open={openDialog === 'publish'}
          />
        </>
      )}
    </PageContainer>
  );
};
