import { useQuery } from '@tanstack/react-query';
import { Alert, Box, Tab, Tabs, Typography } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useResourcePageTitle } from '../../app/useResourcePageTitle';
import { PageContainer } from '../../components/PageContainer';
import { useTabAccessibility } from '../../components/useTabAccessibility';
import { ExtensionOutlet } from '../extensions/ExtensionOutlet';
import { listBlueprintRevisions } from './api';
import { BlueprintDetailHeader } from './BlueprintDetailHeader';
import { BlueprintRevisionComparison } from './BlueprintRevisionComparison';
import { BlueprintVersionMetadata } from './BlueprintVersionMetadata';
import {
  archiveRemovalDisposition,
  blueprintDetailTabs,
  blueprintExtensionContextVersion,
  type BlueprintDetailTab,
} from './constants';
import { MigrationBatchStatus } from './MigrationBatchStatus';
import { PublishBlueprintDialog } from './PublishBlueprintDialog';
import { PublishBlueprintEntitiesDialog } from './PublishBlueprintEntitiesDialog';
import { blueprintQueryKeys } from './queryKeys';
import { RevisionHistory } from './RevisionHistory';
import { SafeMigrationDialog } from './SafeMigrationDialog';
import { useBlueprintDetailActions } from './useBlueprintDetailActions';
import { useBlueprintRevisionComparison } from './useBlueprintRevisionComparison';
import { useSafeBlueprintMigration } from './useSafeBlueprintMigration';
import { lexiconText } from '../lexicon/lexicon';

type BlueprintDetailDialog = 'publish' | 'publishEntities' | 'safeMigration';

export const BlueprintDetailPage = ({
  blueprintId,
}: {
  blueprintId: string;
}) => {
  const { t } = useTranslation();
  const tabId = useTabAccessibility();
  const [pageTab, setPageTab] = useState<BlueprintDetailTab>(
    blueprintDetailTabs.metadata,
  );
  const [openDialog, setOpenDialog] = useState<BlueprintDetailDialog | null>(
    null,
  );
  const closeDialog = () => setOpenDialog(null);
  const { publish, publishEntities, safeMigration } = useBlueprintDetailActions(
    blueprintId,
    {
      onEntitiesPublished: closeDialog,
      onMigrationStarted: () => {
        closeDialog();
        setPageTab(blueprintDetailTabs.migrations);
      },
      onPublished: closeDialog,
    },
  );
  const revisions = useQuery({
    queryKey: blueprintQueryKeys.revisions(blueprintId),
    queryFn: () => listBlueprintRevisions(blueprintId),
  });
  const revisionItems = revisions.data ?? [];
  const comparison = useBlueprintRevisionComparison(blueprintId, revisionItems);
  const migration = useSafeBlueprintMigration(blueprintId, revisionItems);
  const blueprint = revisionItems[0];
  const metadataRevision = comparison.left.data;
  useResourcePageTitle(
    blueprint && lexiconText(blueprint.name),
    t('navigation.blueprints'),
  );

  return (
    <PageContainer>
      {revisions.isPending && (
        <Typography>{t('blueprints.loadingBlueprint')}</Typography>
      )}
      {revisions.isError && (
        <Alert severity="error">{revisions.error.message}</Alert>
      )}
      {blueprint && (
        <>
          <BlueprintDetailHeader
            blueprint={blueprint}
            canStartSafeMigration={migration.canStart}
            migrationActive={migration.currentVersionMigrationActive}
            migrationStatusKnown={migration.migrationBatches.isSuccess}
            onMigrate={() => setOpenDialog('safeMigration')}
            onPublish={() => setOpenDialog('publish')}
            onPublishEntities={() => setOpenDialog('publishEntities')}
          />
          {publish.isError && (
            <Alert severity="error" sx={{ mt: 2 }}>
              {publish.error.message}
            </Alert>
          )}
          {migration.wasEvaluated && !migration.canStart && (
            <Alert severity="info" sx={{ mt: 2 }}>
              {t('blueprints.automaticMigrationUnavailable')}
            </Alert>
          )}
          <Box component="aside" sx={{ mt: 3 }}>
            <ExtensionOutlet
              context={{
                context_version: blueprintExtensionContextVersion,
                blueprint_id: blueprint.id,
                blueprint_version: blueprint.version,
              }}
              outlet="blueprint_panel"
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
              label={t('blueprints.versionMetadata', {
                version: comparison.leftVersion,
              })}
            />
            <Tab
              {...tabId.tab(blueprintDetailTabs.revisionHistory)}
              label={t('blueprints.revisionHistory')}
            />
            <Tab
              {...tabId.tab(blueprintDetailTabs.compareDefinitions)}
              label={t('blueprints.compareDefinitions')}
            />
            <Tab
              {...tabId.tab(blueprintDetailTabs.migrations)}
              label={t('blueprints.migrations')}
            />
          </Tabs>
          {pageTab === blueprintDetailTabs.metadata && metadataRevision && (
            <Box {...tabId.panel(blueprintDetailTabs.metadata)}>
              <BlueprintVersionMetadata blueprint={metadataRevision} />
              <Box component="aside" sx={{ mt: 3 }}>
                <ExtensionOutlet
                  context={{
                    context_version: blueprintExtensionContextVersion,
                    blueprint_id: metadataRevision.blueprint.id,
                    blueprint_version: metadataRevision.blueprint.version,
                  }}
                  outlet="blueprint_detail_panel"
                />
              </Box>
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
          {(safeMigration.isError || publishEntities.isError) && (
            <Alert severity="error" sx={{ mt: 2 }}>
              {safeMigration.error?.message ?? publishEntities.error?.message}
            </Alert>
          )}
          <PublishBlueprintEntitiesDialog
            blueprint={blueprint}
            isPending={publishEntities.isPending}
            onClose={closeDialog}
            onConfirm={(contextId) =>
              publishEntities.mutate({ version: blueprint.version, contextId })
            }
            open={openDialog === 'publishEntities'}
          />
          <SafeMigrationDialog
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
