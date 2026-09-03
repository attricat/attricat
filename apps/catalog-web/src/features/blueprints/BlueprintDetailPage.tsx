import ExpandMoreIcon from '@mui/icons-material/ExpandMore';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Accordion,
  AccordionDetails,
  AccordionSummary,
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
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  TextField,
  Tabs,
  Typography,
} from '@mui/material';
import { lazy, Suspense, useState } from 'react';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import {
  getBlueprintRevision,
  listBlueprintRevisions,
  publishBlueprintRevision,
} from './api';
import { BlueprintViewsPreview } from './BlueprintViewsPreview';
import { formatBlueprintDateTime } from './date-time';
import { blueprintQueryKeys } from './query-keys';
import { RevisionHistory } from './RevisionHistory';

const TomlDiffEditor = lazy(() =>
  import('./TomlDiffEditor').then(({ TomlDiffEditor }) => ({
    default: TomlDiffEditor,
  })),
);

const JsonMetadata = ({ label, value }: { label: string; value: unknown }) => (
  <Box>
    <Typography color="text.secondary" variant="caption">
      {label}
    </Typography>
    <Typography
      component="pre"
      sx={{
        fontFamily: 'monospace',
        fontSize: '0.75rem',
        m: 0,
        overflowX: 'auto',
        whiteSpace: 'pre-wrap',
      }}
    >
      {JSON.stringify(value, null, 2)}
    </Typography>
  </Box>
);

export const BlueprintDetailPage = ({
  blueprintId,
}: {
  blueprintId: string;
}) => {
  const [leftSelection, setLeftSelection] = useState<number | null>(null);
  const [rightSelection, setRightSelection] = useState<number | null>(null);
  const [dataTab, setDataTab] = useState(0);
  const [publishConfirmationOpen, setPublishConfirmationOpen] = useState(false);
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

  return (
    <PageContainer>
      {revisions.isPending && <Typography>Loading blueprint...</Typography>}
      {revisions.isError && (
        <Alert severity="error">{revisions.error.message}</Alert>
      )}
      {blueprint && (
        <>
          <Stack
            alignItems={{ sm: 'center' }}
            direction={{ xs: 'column', sm: 'row' }}
            justifyContent="space-between"
            spacing={2}
          >
            <PageHeader eyebrow="Blueprint" title={blueprint.name} />
            {blueprint.status === 'draft' && (
              <Button
                color="primary"
                onClick={() => setPublishConfirmationOpen(true)}
                variant="contained"
              >
                Publish
              </Button>
            )}
          </Stack>
          <Stack direction="row" spacing={1} sx={{ flexWrap: 'wrap', mt: 1 }}>
            <Chip label={blueprint.code} variant="outlined" />
            <Chip label={blueprint.kind} variant="outlined" />
            <Chip
              color={blueprint.status === 'published' ? 'success' : 'warning'}
              label={blueprint.status}
            />
            <Chip label={`Latest: v${blueprint.version}`} />
          </Stack>
          <Typography color="text.secondary" sx={{ mt: 1.5 }}>
            ID:{' '}
            <Box component="span" sx={{ fontFamily: 'monospace' }}>
              {blueprint.id}
            </Box>
            {' · '}Updated: {formatBlueprintDateTime(blueprint.updated_at)}
          </Typography>
          {publish.isError && (
            <Alert severity="error" sx={{ mt: 2 }}>
              {publish.error.message}
            </Alert>
          )}
          <RevisionHistory revisions={revisionItems} />
          {left.data && (
            <Paper component="section" sx={{ mt: 3, p: 2.5 }}>
              <Typography component="h2" variant="h6">
                Version {left.data.blueprint.version} metadata
              </Typography>
              <Tabs
                allowScrollButtonsMobile
                onChange={(_, value: number) => setDataTab(value)}
                scrollButtons="auto"
                sx={{ mt: 1 }}
                value={dataTab}
                variant="scrollable"
              >
                <Tab label={`Attributes (${left.data.attributes.length})`} />
                <Tab label="Views" />
                <Tab label="View definition" />
                <Tab label="Entity schema" />
                <Tab label="Includes" />
              </Tabs>
              <Box sx={{ mt: 2 }}>
                {dataTab === 4 && (
                  <JsonMetadata
                    label="Includes"
                    value={left.data.blueprint.includes}
                  />
                )}
                {dataTab === 1 && (
                  <BlueprintViewsPreview
                    attributes={left.data.attributes}
                    views={left.data.blueprint.views}
                  />
                )}
                {dataTab === 2 && (
                  <JsonMetadata
                    label="Views"
                    value={left.data.blueprint.views}
                  />
                )}
                {dataTab === 3 && (
                  <JsonMetadata
                    label="Entity schema"
                    value={left.data.blueprint.entity_schema}
                  />
                )}
                {dataTab === 0 && (
                  <Box sx={{ overflowX: 'auto' }}>
                    <Table size="small">
                      <TableHead>
                        <TableRow>
                          <TableCell>Code</TableCell>
                          <TableCell>Type</TableCell>
                          <TableCell>Target</TableCell>
                          <TableCell>Tags</TableCell>
                          <TableCell>Value schema</TableCell>
                          <TableCell>Context</TableCell>
                        </TableRow>
                      </TableHead>
                      <TableBody>
                        {left.data.attributes.map((attribute) => (
                          <TableRow key={attribute.id}>
                            <TableCell>{attribute.code}</TableCell>
                            <TableCell>{attribute.value_type}</TableCell>
                            <TableCell>
                              {attribute.target_blueprint_code ?? '—'}
                            </TableCell>
                            <TableCell>
                              {JSON.stringify(attribute.tags)}
                            </TableCell>
                            <TableCell>
                              {JSON.stringify(attribute.value_schema)}
                            </TableCell>
                            <TableCell>
                              {attribute.context_fallback} /{' '}
                              {attribute.context_editable}
                            </TableCell>
                          </TableRow>
                        ))}
                      </TableBody>
                    </Table>
                  </Box>
                )}
              </Box>
            </Paper>
          )}
          <Accordion component="section" sx={{ mt: 3 }}>
            <AccordionSummary expandIcon={<ExpandMoreIcon />}>
              <Typography component="h2" variant="h6">
                Compare definitions
              </Typography>
            </AccordionSummary>
            <AccordionDetails>
              <Typography color="text.secondary">
                Choose two revisions to inspect the changes between their
                immutable TOML definitions.
              </Typography>
              <Stack
                direction={{ xs: 'column', sm: 'row' }}
                spacing={2}
                sx={{ mt: 2 }}
              >
                <TextField
                  label="Left version"
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
                  label="Right version"
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
                <Suspense fallback={<Typography>Loading editor...</Typography>}>
                  {left.data && right.data && (
                    <TomlDiffEditor
                      modified={right.data.blueprint.definition}
                      modifiedTitle={`Version ${right.data.blueprint.version} · ${right.data.blueprint.status}`}
                      original={left.data.blueprint.definition}
                      originalTitle={`Version ${left.data.blueprint.version} · ${left.data.blueprint.status}`}
                    />
                  )}
                </Suspense>
              </Box>
            </AccordionDetails>
          </Accordion>
          <Dialog
            onClose={() =>
              !publish.isPending && setPublishConfirmationOpen(false)
            }
            open={publishConfirmationOpen}
          >
            <DialogTitle>Publish blueprint?</DialogTitle>
            <DialogContent>
              <DialogContentText>
                Publish {blueprint.name} version {blueprint.version}? Published
                blueprints are available for creating entities.
              </DialogContentText>
            </DialogContent>
            <DialogActions>
              <Button
                disabled={publish.isPending}
                onClick={() => setPublishConfirmationOpen(false)}
              >
                Cancel
              </Button>
              <Button
                autoFocus
                disabled={publish.isPending}
                onClick={() => publish.mutate(blueprint.version)}
                variant="contained"
              >
                {publish.isPending ? 'Publishing…' : 'Publish'}
              </Button>
            </DialogActions>
          </Dialog>
        </>
      )}
    </PageContainer>
  );
};
