import { useQuery } from '@tanstack/react-query';
import {
  Alert,
  Box,
  Chip,
  Container,
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
import { getBlueprintRevision, listBlueprintRevisions } from './api';
import { BlueprintViewsPreview } from './BlueprintViewsPreview';
import { blueprintQueryKeys } from './query-keys';

const TomlEditor = lazy(() =>
  import('./TomlEditor').then(({ TomlEditor }) => ({ default: TomlEditor })),
);

const dateTime = (value: string | null) =>
  value
    ? new Intl.DateTimeFormat(undefined, {
        dateStyle: 'medium',
        timeStyle: 'short',
      }).format(new Date(value))
    : 'Not published';

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
  const revisions = useQuery({
    queryKey: blueprintQueryKeys.revisions(blueprintId),
    queryFn: () => listBlueprintRevisions(blueprintId),
  });
  const revisionItems = revisions.data ?? [];
  const leftVersion = leftSelection ?? revisionItems[0]?.version;
  const rightVersion =
    rightSelection ?? revisionItems[1]?.version ?? leftVersion;
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
    <Container component="main" maxWidth="xl" sx={{ py: { xs: 4, md: 7 } }}>
      {revisions.isPending && <Typography>Loading blueprint...</Typography>}
      {revisions.isError && (
        <Alert severity="error">{revisions.error.message}</Alert>
      )}
      {blueprint && (
        <>
          <Typography
            color="primary"
            sx={{
              fontWeight: 700,
              letterSpacing: '.12em',
              textTransform: 'uppercase',
            }}
            variant="overline"
          >
            Blueprint
          </Typography>
          <Typography component="h1" variant="h2">
            {blueprint.name}
          </Typography>
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
            {' · '}Updated: {dateTime(blueprint.updated_at)}
          </Typography>
          <Paper component="section" sx={{ mt: 3, p: 2.5 }}>
            <Typography component="h2" variant="h6">
              Revision history
            </Typography>
            <Box sx={{ overflowX: 'auto', mt: 1 }}>
              <Table size="small">
                <TableHead>
                  <TableRow>
                    <TableCell>Version</TableCell>
                    <TableCell>Status</TableCell>
                    <TableCell>Created</TableCell>
                    <TableCell>Published</TableCell>
                    <TableCell>Definition hash</TableCell>
                  </TableRow>
                </TableHead>
                <TableBody>
                  {revisionItems.map((revision) => (
                    <TableRow key={revision.version}>
                      <TableCell>v{revision.version}</TableCell>
                      <TableCell>
                        <Chip
                          color={
                            revision.status === 'published'
                              ? 'success'
                              : 'warning'
                          }
                          label={revision.status}
                          size="small"
                        />
                      </TableCell>
                      <TableCell>{dateTime(revision.created_at)}</TableCell>
                      <TableCell>{dateTime(revision.published_at)}</TableCell>
                      <TableCell sx={{ fontFamily: 'monospace' }}>
                        {revision.definition_hash}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </Box>
          </Paper>
          <Paper component="section" sx={{ mt: 3, p: 2.5 }}>
            <Typography component="h2" variant="h6">
              Compare definitions
            </Typography>
            <Typography color="text.secondary" sx={{ mt: 0.5 }}>
              Choose two revisions to inspect their immutable TOML definitions
              side by side.
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
            <Box
              sx={{
                display: 'grid',
                gap: 2,
                gridTemplateColumns: { xs: '1fr', lg: '1fr 1fr' },
                mt: 3,
              }}
            >
              <Suspense fallback={<Typography>Loading editor...</Typography>}>
                {left.data && (
                  <TomlEditor
                    title={`Version ${left.data.blueprint.version} · ${left.data.blueprint.status}`}
                    value={left.data.blueprint.definition}
                  />
                )}
                {right.data && (
                  <TomlEditor
                    title={`Version ${right.data.blueprint.version} · ${right.data.blueprint.status}`}
                    value={right.data.blueprint.definition}
                  />
                )}
              </Suspense>
            </Box>
          </Paper>
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
                <Tab label="Includes" />
                <Tab label="Display" />
                <Tab label="Views" />
                <Tab label="View definition" />
                <Tab label="Entity schema" />
                <Tab label={`Attributes (${left.data.attributes.length})`} />
              </Tabs>
              <Box sx={{ mt: 2 }}>
                {dataTab === 0 && (
                  <JsonMetadata
                    label="Includes"
                    value={left.data.blueprint.includes}
                  />
                )}
                {dataTab === 1 && (
                  <JsonMetadata
                    label="Display"
                    value={left.data.blueprint.display}
                  />
                )}
                {dataTab === 2 && (
                  <BlueprintViewsPreview
                    attributes={left.data.attributes}
                    views={left.data.blueprint.views}
                  />
                )}
                {dataTab === 3 && (
                  <JsonMetadata
                    label="Views"
                    value={left.data.blueprint.views}
                  />
                )}
                {dataTab === 4 && (
                  <JsonMetadata
                    label="Entity schema"
                    value={left.data.blueprint.entity_schema}
                  />
                )}
                {dataTab === 5 && (
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
        </>
      )}
    </Container>
  );
};
