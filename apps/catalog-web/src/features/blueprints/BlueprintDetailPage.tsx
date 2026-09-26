import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import {
  Alert,
  Box,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  MenuItem,
  Paper,
  Stack,
  Tab,
  TextField,
  Tabs,
  Typography,
} from '@mui/material';
import { lazy, Suspense, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { useTabAccessibility } from '../../components/useTabAccessibility';
import {
  getBlueprintRevision,
  getSafeBlueprintMigrationImpact,
  listBlueprintMigrationBatches,
  listBlueprintRevisions,
  publishBlueprintEntities,
  publishBlueprintEntitiesAllChannels,
  publishBlueprintRevision,
  startSafeBlueprintMigrationBatch,
} from './api';
import { formatBlueprintDateTime } from './dateTime';
import {
  ACTIVE_MIGRATION_POLL_INTERVAL_MS,
  hasActiveMigrationForVersion,
  isMigrationBatchActive,
} from './migrationBatches';
import { blueprintQueryKeys } from './queryKeys';
import { RevisionHistory } from './RevisionHistory';
import { BlueprintVersionMetadata } from './BlueprintVersionMetadata';
import { MigrationBatchStatus } from './MigrationBatchStatus';
import { ExtensionOutlet } from '../extensions/ExtensionOutlet';
import { listPublicationChannels } from '../exports/api';
import { exportQueryKeys } from '../exports/queryKeys';
import { entityQueryKeys } from '../entities/queryKeys';

const TomlDiffEditor = lazy(() =>
  import('./TomlDiffEditor').then(({ TomlDiffEditor }) => ({
    default: TomlDiffEditor,
  })),
);

export const BlueprintDetailPage = ({
  blueprintId,
}: {
  blueprintId: string;
}) => {
  const { t } = useTranslation();
  const tabId = useTabAccessibility();
  const [leftSelection, setLeftSelection] = useState<number | null>(null);
  const [rightSelection, setRightSelection] = useState<number | null>(null);
  const [pageTab, setPageTab] = useState(0);
  const [publishConfirmationOpen, setPublishConfirmationOpen] = useState(false);
  const [safeMigrationConfirmationOpen, setSafeMigrationConfirmationOpen] =
    useState(false);
  const [entityPublicationOpen, setEntityPublicationOpen] = useState(false);
  const [publicationChannel, setPublicationChannel] = useState('all');
  const queryClient = useQueryClient();
  const publish = useMutation({
    mutationFn: (version: number) =>
      publishBlueprintRevision(blueprintId, version),
    onSuccess: async (_, version) => {
      setPublishConfirmationOpen(false);
      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: blueprintQueryKeys.catalogue(),
        }),
        queryClient.invalidateQueries({
          queryKey: blueprintQueryKeys.revisions(blueprintId),
        }),
        queryClient.invalidateQueries({
          queryKey: blueprintQueryKeys.revision(blueprintId, version),
        }),
      ]);
    },
  });
  const safeMigration = useMutation({
    mutationFn: ({
      version,
      removalDisposition,
    }: {
      version: number;
      removalDisposition?: 'archive';
    }) =>
      startSafeBlueprintMigrationBatch(
        blueprintId,
        version,
        removalDisposition,
      ),
    onSuccess: async () => {
      setSafeMigrationConfirmationOpen(false);
      setPageTab(3);
      await queryClient.invalidateQueries({
        queryKey: blueprintQueryKeys.migrationBatches(blueprintId),
      });
    },
  });
  const publicationChannels = useQuery({
    queryKey: exportQueryKeys.channels(),
    queryFn: listPublicationChannels,
  });
  const publishEntities = useMutation({
    mutationFn: ({
      version,
      contextId,
    }: {
      version: number;
      contextId: string;
    }) =>
      contextId === 'all'
        ? publishBlueprintEntitiesAllChannels(blueprintId, version)
        : publishBlueprintEntities(blueprintId, version, contextId),
    onSuccess: async () => {
      setEntityPublicationOpen(false);
      await queryClient.invalidateQueries({
        queryKey: entityQueryKeys.publications(),
      });
    },
  });
  const revisions = useQuery({
    queryKey: blueprintQueryKeys.revisions(blueprintId),
    queryFn: () => listBlueprintRevisions(blueprintId),
  });
  const revisionItems = revisions.data ?? [];
  const leftVersion =
    leftSelection ?? revisionItems[1]?.version ?? revisionItems[0]?.version;
  const rightVersion =
    rightSelection ?? revisionItems[0]?.version ?? leftVersion;
  const left = useQuery({
    queryKey: blueprintQueryKeys.revision(blueprintId, leftVersion ?? 0),
    queryFn: () => getBlueprintRevision(blueprintId, leftVersion!),
    enabled: leftVersion !== undefined,
  });
  const right = useQuery({
    queryKey: blueprintQueryKeys.revision(blueprintId, rightVersion ?? 0),
    queryFn: () => getBlueprintRevision(blueprintId, rightVersion!),
    enabled: rightVersion !== undefined,
  });
  const blueprint = revisionItems[0];
  const latestPublished = revisionItems.find(
    (revision) => revision.status === 'published',
  );
  const migrationBatches = useQuery({
    queryKey: blueprintQueryKeys.migrationBatches(blueprintId),
    queryFn: () => listBlueprintMigrationBatches(blueprintId),
    refetchInterval: (query) =>
      query.state.data?.some(isMigrationBatchActive)
        ? ACTIVE_MIGRATION_POLL_INTERVAL_MS
        : false,
  });
  const currentVersionMigrationActive = hasActiveMigrationForVersion(
    migrationBatches.data,
    latestPublished?.version,
  );
  const safeMigrationSourceVersion = latestPublished
    ? revisionItems.find(
        (revision) =>
          revision.version < latestPublished.version &&
          revision.status === 'published',
      )?.version
    : undefined;
  const safeMigrationSource = useQuery({
    queryKey: blueprintQueryKeys.revision(
      blueprintId,
      safeMigrationSourceVersion ?? 0,
    ),
    queryFn: () =>
      getBlueprintRevision(blueprintId, safeMigrationSourceVersion!),
    enabled: safeMigrationSourceVersion !== undefined,
  });
  const safeMigrationTarget = useQuery({
    queryKey: blueprintQueryKeys.revision(
      blueprintId,
      latestPublished?.version ?? 0,
    ),
    queryFn: () => getBlueprintRevision(blueprintId, latestPublished!.version),
    enabled: latestPublished !== undefined,
  });
  const canStartSafeMigrationBase =
    latestPublished !== undefined &&
    safeMigrationSource.data !== undefined &&
    safeMigrationTarget.data !== undefined &&
    isSafeAutomaticMigration(
      safeMigrationSource.data,
      safeMigrationTarget.data,
    );
  const safeMigrationImpact = useQuery({
    queryKey: [
      ...blueprintQueryKeys.revision(
        blueprintId,
        latestPublished?.version ?? 0,
      ),
      'safe-migration-impact',
    ],
    queryFn: () =>
      getSafeBlueprintMigrationImpact(blueprintId, latestPublished!.version),
    enabled: latestPublished !== undefined && canStartSafeMigrationBase,
  });
  const canStartSafeMigration =
    canStartSafeMigrationBase && safeMigrationImpact.data !== undefined;
  const safeMigrationWasEvaluated =
    latestPublished !== undefined &&
    safeMigrationSource.isSuccess &&
    safeMigrationTarget.isSuccess;

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
          <PageHeader
            actions={
              <Stack direction="row" spacing={1}>
                <Link
                  params={{
                    blueprintId,
                    version: String(blueprint.version),
                  }}
                  to="/manage/blueprints/$blueprintId/revisions/$version/new"
                >
                  <Button variant="outlined">
                    {t('blueprints.editBlueprint')}
                  </Button>
                </Link>
                {blueprint.status === 'draft' && (
                  <Button
                    color="primary"
                    onClick={() => setPublishConfirmationOpen(true)}
                    variant="contained"
                  >
                    {t('blueprints.publish')}
                  </Button>
                )}
                {blueprint.status === 'published' && (
                  <Button
                    onClick={() => setEntityPublicationOpen(true)}
                    variant="outlined"
                  >
                    {t('blueprints.publishEntities')}
                  </Button>
                )}
                {canStartSafeMigration && (
                  <Button
                    disabled={
                      !migrationBatches.isSuccess ||
                      currentVersionMigrationActive
                    }
                    onClick={() => setSafeMigrationConfirmationOpen(true)}
                    variant="contained"
                  >
                    {currentVersionMigrationActive
                      ? t('blueprints.migrationInProgress')
                      : t('blueprints.migrateCompatibleEntities')}
                  </Button>
                )}
              </Stack>
            }
            eyebrow={t('blueprints.blueprint')}
            title={blueprint.name}
          />
          <Stack direction="row" spacing={1} sx={{ flexWrap: 'wrap', mt: 1 }}>
            <Chip label={blueprint.code} variant="outlined" />
            <Chip label={blueprint.kind} variant="outlined" />
            <Chip
              color={blueprint.status === 'published' ? 'success' : 'warning'}
              label={blueprint.status}
            />
            <Chip
              label={t('blueprints.latestVersion', {
                version: blueprint.version,
              })}
            />
          </Stack>
          <Typography color="text.secondary" sx={{ mt: 1.5 }}>
            {t('blueprints.id')}:{' '}
            <Box component="span" sx={{ fontFamily: 'monospace' }}>
              {blueprint.id}
            </Box>
            {' · '}
            {t('blueprints.updated')}:{' '}
            {formatBlueprintDateTime(
              blueprint.updated_at,
              t('blueprints.notPublished'),
            )}
          </Typography>
          {publish.isError && (
            <Alert severity="error" sx={{ mt: 2 }}>
              {publish.error.message}
            </Alert>
          )}
          {safeMigrationWasEvaluated && !canStartSafeMigration && (
            <Alert severity="info" sx={{ mt: 2 }}>
              {t('blueprints.automaticMigrationUnavailable')}
            </Alert>
          )}
          <Tabs
            allowScrollButtonsMobile
            onChange={(_, value: number) => setPageTab(value)}
            scrollButtons="auto"
            sx={{ mt: 3 }}
            value={pageTab}
            variant="scrollable"
          >
            <Tab
              {...tabId.tab(0)}
              label={t('blueprints.versionMetadata', {
                version: leftVersion,
              })}
            />
            <Tab {...tabId.tab(1)} label={t('blueprints.revisionHistory')} />
            <Tab {...tabId.tab(2)} label={t('blueprints.compareDefinitions')} />
            <Tab {...tabId.tab(3)} label={t('blueprints.migrations')} />
          </Tabs>
          {pageTab === 0 && left.data && (
            <Box {...tabId.panel(0)}>
              <BlueprintVersionMetadata blueprint={left.data} />
              <Box component="aside" sx={{ mt: 3 }}>
                <ExtensionOutlet
                  context={{
                    context_version: 1,
                    blueprint_id: left.data.blueprint.id,
                    blueprint_version: left.data.blueprint.version,
                  }}
                  outlet="blueprint_detail_panel"
                />
              </Box>
            </Box>
          )}
          {pageTab === 1 && (
            <Box {...tabId.panel(1)}>
              <RevisionHistory
                blueprintId={blueprintId}
                revisions={revisionItems}
              />
            </Box>
          )}
          {pageTab === 2 && (
            <Paper
              {...tabId.panel(2)}
              component="section"
              sx={{ mt: 3, p: 2.5 }}
            >
              <Typography component="h2" variant="h6">
                {t('blueprints.compareDefinitions')}
              </Typography>
              <Typography color="text.secondary" sx={{ mt: 1 }}>
                {t('blueprints.compareDefinitionsDescription')}
              </Typography>
              <Stack
                direction={{ xs: 'column', sm: 'row' }}
                spacing={2}
                sx={{ mt: 2 }}
              >
                <TextField
                  label={t('blueprints.leftVersion')}
                  onChange={(event) =>
                    setLeftSelection(Number(event.target.value))
                  }
                  select
                  value={leftVersion ?? ''}
                >
                  {revisionItems.map((revision) => (
                    <MenuItem key={revision.version} value={revision.version}>
                      v{revision.version} ({revision.status})
                    </MenuItem>
                  ))}
                </TextField>
                <TextField
                  label={t('blueprints.rightVersion')}
                  onChange={(event) =>
                    setRightSelection(Number(event.target.value))
                  }
                  select
                  value={rightVersion ?? ''}
                >
                  {revisionItems.map((revision) => (
                    <MenuItem key={revision.version} value={revision.version}>
                      v{revision.version} ({revision.status})
                    </MenuItem>
                  ))}
                </TextField>
              </Stack>
              {(left.isError || right.isError) && (
                <Alert severity="error" sx={{ mt: 2 }}>
                  {left.error?.message ?? right.error?.message}
                </Alert>
              )}
              <Box sx={{ mt: 3 }}>
                <Suspense
                  fallback={
                    <Typography>{t('blueprints.loadingEditor')}</Typography>
                  }
                >
                  {left.data && right.data && (
                    <TomlDiffEditor
                      modified={right.data.blueprint.definition}
                      modifiedTitle={t('blueprints.revisionTitle', {
                        version: right.data.blueprint.version,
                        status: right.data.blueprint.status,
                      })}
                      original={left.data.blueprint.definition}
                      originalTitle={t('blueprints.revisionTitle', {
                        version: left.data.blueprint.version,
                        status: left.data.blueprint.status,
                      })}
                    />
                  )}
                </Suspense>
              </Box>
            </Paper>
          )}
          {pageTab === 3 && (
            <Box {...tabId.panel(3)}>
              <MigrationBatchStatus batches={migrationBatches} />
            </Box>
          )}
          {(safeMigration.isError || publishEntities.isError) && (
            <Alert severity="error" sx={{ mt: 2 }}>
              {safeMigration.error?.message ?? publishEntities.error?.message}
            </Alert>
          )}
          <Dialog
            onClose={() =>
              !publishEntities.isPending && setEntityPublicationOpen(false)
            }
            open={entityPublicationOpen}
          >
            <DialogTitle>{t('blueprints.publishEntities')}</DialogTitle>
            <DialogContent>
              <DialogContentText>
                {t('blueprints.publishEntitiesDescription', {
                  name: blueprint.name,
                  version: blueprint.version,
                })}
              </DialogContentText>
              <TextField
                fullWidth
                label={t('blueprints.publicationChannel')}
                onChange={(event) => setPublicationChannel(event.target.value)}
                select
                sx={{ mt: 2 }}
                value={publicationChannel}
              >
                <MenuItem value="all">
                  {t('blueprints.allEnabledChannels')}
                </MenuItem>
                {publicationChannels.data
                  ?.filter((channel) => channel.enabled)
                  .map((channel) => (
                    <MenuItem
                      key={channel.context_id}
                      value={channel.context_id}
                    >
                      {channel.context_code}
                    </MenuItem>
                  ))}
              </TextField>
            </DialogContent>
            <DialogActions>
              <Button
                disabled={publishEntities.isPending}
                onClick={() => setEntityPublicationOpen(false)}
              >
                {t('blueprints.cancel')}
              </Button>
              <Button
                disabled={publishEntities.isPending}
                onClick={() =>
                  publishEntities.mutate({
                    version: blueprint.version,
                    contextId: publicationChannel,
                  })
                }
                variant="contained"
              >
                {publishEntities.isPending
                  ? t('blueprints.publishing')
                  : t('blueprints.publish')}
              </Button>
            </DialogActions>
          </Dialog>
          <Dialog
            onClose={() =>
              !safeMigration.isPending &&
              setSafeMigrationConfirmationOpen(false)
            }
            open={safeMigrationConfirmationOpen}
          >
            <DialogTitle>
              {t('blueprints.migrateCompatibleEntities')}
            </DialogTitle>
            <DialogContent>
              <DialogContentText>
                {t('blueprints.migrateCompatibleEntitiesDescription', {
                  source: safeMigrationSourceVersion,
                  target: latestPublished?.version,
                })}
              </DialogContentText>
              {safeMigrationImpact.data && (
                <DialogContentText sx={{ mt: 2 }}>
                  {t('blueprints.migrationImpact', {
                    entities: safeMigrationImpact.data.eligible_entities,
                    values: safeMigrationImpact.data.removed_values,
                    affectedEntities:
                      safeMigrationImpact.data.entities_with_removed_values,
                    attributes:
                      safeMigrationImpact.data.removed_attribute_codes.join(
                        ', ',
                      ),
                  })}
                </DialogContentText>
              )}
              {safeMigrationImpact.data?.requires_removal_disposition && (
                <Alert severity="warning" sx={{ mt: 2 }}>
                  {t('blueprints.archiveRemovedValues')}
                </Alert>
              )}
            </DialogContent>
            <DialogActions>
              <Button
                disabled={safeMigration.isPending}
                onClick={() => setSafeMigrationConfirmationOpen(false)}
              >
                {t('blueprints.cancel')}
              </Button>
              <Button
                disabled={safeMigration.isPending}
                onClick={() =>
                  safeMigration.mutate({
                    version: latestPublished!.version,
                    removalDisposition: safeMigrationImpact.data
                      ?.requires_removal_disposition
                      ? 'archive'
                      : undefined,
                  })
                }
                variant="contained"
              >
                {safeMigration.isPending
                  ? t('blueprints.startingMigration')
                  : t('blueprints.startMigration')}
              </Button>
            </DialogActions>
          </Dialog>
          <Dialog
            onClose={() =>
              !publish.isPending && setPublishConfirmationOpen(false)
            }
            open={publishConfirmationOpen}
          >
            <DialogTitle>{t('blueprints.publishBlueprintTitle')}</DialogTitle>
            <DialogContent>
              <DialogContentText>
                {t('blueprints.publishBlueprintDescription', {
                  name: blueprint.name,
                  version: blueprint.version,
                })}
              </DialogContentText>
            </DialogContent>
            <DialogActions>
              <Button
                disabled={publish.isPending}
                onClick={() => setPublishConfirmationOpen(false)}
              >
                {t('blueprints.cancel')}
              </Button>
              <Button
                disabled={publish.isPending}
                onClick={() => publish.mutate(blueprint.version)}
                variant="contained"
              >
                {publish.isPending
                  ? t('blueprints.publishing')
                  : t('blueprints.publish')}
              </Button>
            </DialogActions>
          </Dialog>
        </>
      )}
    </PageContainer>
  );
};

const isSafeAutomaticMigration = (
  source: Awaited<ReturnType<typeof getBlueprintRevision>>,
  target: Awaited<ReturnType<typeof getBlueprintRevision>>,
) => {
  if (
    JSON.stringify(source.blueprint.entity_schema) !==
    JSON.stringify(target.blueprint.entity_schema)
  ) {
    return false;
  }
  const targetAttributes = new Map(
    target.attributes.map((attribute) => [attribute.code, attribute]),
  );
  return source.attributes.every((attribute) => {
    const targetAttribute = targetAttributes.get(attribute.code);
    return (
      targetAttribute === undefined ||
      (attribute.value_type === targetAttribute.value_type &&
        JSON.stringify(attribute.value_schema) ===
          JSON.stringify(targetAttribute.value_schema) &&
        JSON.stringify(attribute.default_value) ===
          JSON.stringify(targetAttribute.default_value) &&
        JSON.stringify(attribute.file_policy) ===
          JSON.stringify(targetAttribute.file_policy) &&
        attribute.target_blueprint_code ===
          targetAttribute.target_blueprint_code &&
        attribute.cardinality === targetAttribute.cardinality &&
        attribute.target_cardinality === targetAttribute.target_cardinality &&
        attribute.context_fallback === targetAttribute.context_fallback &&
        attribute.context_editable === targetAttribute.context_editable &&
        attribute.readonly === targetAttribute.readonly)
    );
  });
};
