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
import {
  getBlueprintRevision,
  listBlueprintRevisions,
  publishBlueprintRevision,
  startSafeBlueprintMigrationBatch,
} from './api';
import { formatBlueprintDateTime } from './date-time';
import { blueprintQueryKeys } from './query-keys';
import { RevisionHistory } from './RevisionHistory';
import { BlueprintVersionMetadata } from './BlueprintVersionMetadata';
import { ExtensionOutlet } from '../extensions/ExtensionOutlet';

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
  const [leftSelection, setLeftSelection] = useState<number | null>(null);
  const [rightSelection, setRightSelection] = useState<number | null>(null);
  const [pageTab, setPageTab] = useState(0);
  const [publishConfirmationOpen, setPublishConfirmationOpen] = useState(false);
  const [safeMigrationConfirmationOpen, setSafeMigrationConfirmationOpen] =
    useState(false);
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
    mutationFn: (version: number) =>
      startSafeBlueprintMigrationBatch(blueprintId, version),
    onSuccess: () => setSafeMigrationConfirmationOpen(false),
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
  const safeMigrationSourceVersion = latestPublished
    ? latestPublished.version - 1
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
  const canStartSafeMigration =
    latestPublished !== undefined &&
    safeMigrationSource.data !== undefined &&
    safeMigrationTarget.data !== undefined &&
    isSafeAutomaticMigration(
      safeMigrationSource.data,
      safeMigrationTarget.data,
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
                {canStartSafeMigration && (
                  <Button
                    onClick={() => setSafeMigrationConfirmationOpen(true)}
                    variant="contained"
                  >
                    {t('blueprints.migrateCompatibleEntities')}
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
          <Tabs
            allowScrollButtonsMobile
            onChange={(_, value: number) => setPageTab(value)}
            scrollButtons="auto"
            sx={{ mt: 3 }}
            value={pageTab}
            variant="scrollable"
          >
            <Tab
              label={t('blueprints.versionMetadata', {
                version: leftVersion,
              })}
            />
            <Tab label={t('blueprints.revisionHistory')} />
            <Tab label={t('blueprints.compareDefinitions')} />
          </Tabs>
          {pageTab === 1 && (
            <RevisionHistory
              blueprintId={blueprintId}
              revisions={revisionItems}
            />
          )}
          {pageTab === 0 && left.data && (
            <BlueprintVersionMetadata blueprint={left.data} />
          )}
          {pageTab === 0 && left.data && (
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
          )}
          {pageTab === 2 && (
            <Paper component="section" sx={{ mt: 3, p: 2.5 }}>
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
          {safeMigration.isError && (
            <Alert severity="error" sx={{ mt: 2 }}>
              {safeMigration.error.message}
            </Alert>
          )}
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
                onClick={() => safeMigration.mutate(latestPublished!.version)}
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
      targetAttribute !== undefined &&
      attribute.value_type === targetAttribute.value_type &&
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
      attribute.readonly === targetAttribute.readonly
    );
  });
};
