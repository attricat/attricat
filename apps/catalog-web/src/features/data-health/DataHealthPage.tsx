import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import {
  Accordion,
  AccordionDetails,
  AccordionSummary,
  Alert,
  Box,
  Chip,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import ExpandMoreIcon from '@mui/icons-material/ExpandMore';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import {
  getDataHealthBlueprints,
  getDataHealthCompleteness,
  getDataHealthContexts,
  getDataHealthFreshness,
  getDataHealthRelationships,
  getDataHealthStorage,
  getDataHealthSummary,
  refreshDataHealth,
} from './api';
import { dataHealthQueryKeys } from './query-keys';
import { DataHealthControls } from './DataHealthControls';
import type { DataHealthSearch } from './schemas';

const formatDate = (value: string | null) =>
  value
    ? new Intl.DateTimeFormat(undefined, { dateStyle: 'medium' }).format(
        new Date(value),
      )
    : 'Never';

const formatBytes = (bytes: number) => {
  if (bytes < 1024) return `${bytes} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  const unit = Math.min(
    Math.floor(Math.log(bytes) / Math.log(1024)),
    units.length,
  );
  return `${(bytes / 1024 ** unit).toFixed(1)} ${units[unit - 1]}`;
};

const SectionError = ({ error }: { error: Error | null }) =>
  error ? <Alert severity="error">{error.message}</Alert> : null;

export const DataHealthPage = ({ search }: { search: DataHealthSearch }) => {
  const staleAfterDays = search.staleAfterDays ?? 90;
  const navigate = useNavigate({ from: '/data-health' });
  const queryClient = useQueryClient();
  const [showCompleteness, setShowCompleteness] = useState(false);
  const [showContexts, setShowContexts] = useState(false);
  const [showRelationships, setShowRelationships] = useState(false);
  const summary = useQuery({
    queryKey: dataHealthQueryKeys.summary(staleAfterDays),
    queryFn: () => getDataHealthSummary(staleAfterDays),
    staleTime: 30_000,
  });
  const blueprints = useQuery({
    queryKey: dataHealthQueryKeys.blueprints(staleAfterDays),
    queryFn: () => getDataHealthBlueprints(staleAfterDays),
    staleTime: 30_000,
  });
  const freshness = useQuery({
    queryKey: dataHealthQueryKeys.freshness(),
    queryFn: getDataHealthFreshness,
    staleTime: 30_000,
  });
  const storage = useQuery({
    queryKey: dataHealthQueryKeys.storage(),
    queryFn: getDataHealthStorage,
    staleTime: 30_000,
  });
  const completeness = useQuery({
    queryKey: dataHealthQueryKeys.completeness(),
    queryFn: getDataHealthCompleteness,
    enabled: showCompleteness,
    staleTime: 30_000,
  });
  const contexts = useQuery({
    queryKey: dataHealthQueryKeys.contexts(),
    queryFn: getDataHealthContexts,
    enabled: showContexts,
    staleTime: 30_000,
  });
  const relationships = useQuery({
    queryKey: dataHealthQueryKeys.relationships(),
    queryFn: getDataHealthRelationships,
    enabled: showRelationships,
    staleTime: 30_000,
  });
  const refresh = useMutation({
    mutationFn: refreshDataHealth,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['data-health'] }),
  });

  return (
    <PageContainer>
      <PageHeader
        actions={
          <DataHealthControls
            isRefreshing={refresh.isPending}
            onRefresh={() => refresh.mutate()}
            onStaleAfterDaysChange={(days) => {
              void navigate({ search: { staleAfterDays: days } });
            }}
            refreshError={refresh.error}
            staleAfterDays={staleAfterDays}
          />
        }
        description="Monitor catalog currency, freshness, correctness, and storage growth."
        title="Data health"
      />
      <SectionError error={summary.error} />
      {summary.data && (
        <Stack
          direction={{ xs: 'column', sm: 'row' }}
          spacing={2}
          sx={{ flexWrap: 'wrap', mt: 4 }}
        >
          {[
            ['Outdated entities', summary.data.outdated_entities],
            ['Active entities', summary.data.active_entities],
            [`Stale after ${staleAfterDays} days`, summary.data.stale_entities],
            [
              'Deleted relationship targets',
              summary.data.deleted_relationship_targets,
            ],
          ].map(([label, value]) => (
            <Paper key={String(label)} sx={{ minWidth: 190, p: 2 }}>
              <Typography color="text.secondary" variant="body2">
                {label}
              </Typography>
              <Typography variant="h4">{value}</Typography>
            </Paper>
          ))}
        </Stack>
      )}
      <Paper sx={{ mt: 4, p: 2 }}>
        <Typography variant="h5">Storage</Typography>
        <SectionError error={storage.error} />
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell>Table</TableCell>
              <TableCell align="right">Total size</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {(storage.data ?? []).map((item) => (
              <TableRow key={item.table}>
                <TableCell>{item.table}</TableCell>
                <TableCell align="right">{formatBytes(item.bytes)}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </Paper>
      <Paper sx={{ mt: 4, overflowX: 'auto' }}>
        <Box sx={{ p: 2 }}>
          <Typography variant="h5">Blueprint health</Typography>
        </Box>
        <SectionError error={blueprints.error} />
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell>Blueprint</TableCell>
              <TableCell>Entities</TableCell>
              <TableCell>Outdated</TableCell>
              <TableCell>Stale</TableCell>
              <TableCell>Oldest update</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {(blueprints.data ?? []).map((blueprint) => (
              <TableRow key={blueprint.code}>
                <TableCell>
                  <Stack
                    direction="row"
                    spacing={1}
                    sx={{ alignItems: 'center' }}
                  >
                    <Typography variant="body2">
                      {blueprint.name} ({blueprint.code}) v
                      {blueprint.current_version}
                    </Typography>
                    {blueprint.outdated_entities > 0 && (
                      <Chip
                        color="warning"
                        label={`${blueprint.outdated_entities} outdated`}
                        size="small"
                      />
                    )}
                  </Stack>
                </TableCell>
                <TableCell>{blueprint.active_entities}</TableCell>
                <TableCell>{blueprint.outdated_entities}</TableCell>
                <TableCell>{blueprint.stale_entities}</TableCell>
                <TableCell>{formatDate(blueprint.oldest_updated_at)}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </Paper>
      <Paper sx={{ mt: 4, p: 2 }}>
        <Typography variant="h5">Freshness distribution</Typography>
        <SectionError error={freshness.error} />
        <Stack direction="row" spacing={3} sx={{ mt: 2 }}>
          {(freshness.data ?? []).map((band) => (
            <Box key={band.label}>
              <Typography variant="h5">{band.entities}</Typography>
              <Typography color="text.secondary" variant="body2">
                {band.label}
              </Typography>
            </Box>
          ))}
        </Stack>
      </Paper>
      <Accordion
        onChange={(_, expanded) => setShowCompleteness(expanded)}
        sx={{ mt: 4 }}
      >
        <AccordionSummary expandIcon={<ExpandMoreIcon />}>
          <Typography variant="h5">Default completeness</Typography>
        </AccordionSummary>
        <AccordionDetails>
          <Typography color="text.secondary" sx={{ mb: 2 }}>
            Complete entities provide every schema-required field in the default
            context.
          </Typography>
          <SectionError error={completeness.error} />
          <Table size="small">
            <TableHead>
              <TableRow>
                <TableCell>Blueprint</TableCell>
                <TableCell>Active entities</TableCell>
                <TableCell>Default complete</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {(completeness.data ?? []).map((item) => (
                <TableRow key={item.code}>
                  <TableCell>
                    <Stack
                      direction="row"
                      spacing={1}
                      sx={{ alignItems: 'center' }}
                    >
                      <Typography variant="body2">
                        {item.name} ({item.code}) v{item.current_version}
                      </Typography>
                      {item.outdated_entities > 0 && (
                        <Chip
                          color="warning"
                          label={`${item.outdated_entities} outdated`}
                          size="small"
                        />
                      )}
                    </Stack>
                  </TableCell>
                  <TableCell>{item.active_entities}</TableCell>
                  <TableCell>{item.default_complete_entities}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </AccordionDetails>
      </Accordion>
      <Accordion onChange={(_, expanded) => setShowContexts(expanded)}>
        <AccordionSummary expandIcon={<ExpandMoreIcon />}>
          <Typography variant="h5">Context coverage</Typography>
        </AccordionSummary>
        <AccordionDetails>
          <SectionError error={contexts.error} />
          <Table size="small">
            <TableHead>
              <TableRow>
                <TableCell>Context</TableCell>
                <TableCell>Entities with direct values</TableCell>
                <TableCell>Current direct values</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {(contexts.data ?? []).map((item) => (
                <TableRow key={item.code}>
                  <TableCell>{item.code}</TableCell>
                  <TableCell>{item.direct_entities}</TableCell>
                  <TableCell>{item.direct_values}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </AccordionDetails>
      </Accordion>
      <Accordion onChange={(_, expanded) => setShowRelationships(expanded)}>
        <AccordionSummary expandIcon={<ExpandMoreIcon />}>
          <Typography variant="h5">Relationship integrity</Typography>
        </AccordionSummary>
        <AccordionDetails>
          <SectionError error={relationships.error} />
          <Table size="small">
            <TableHead>
              <TableRow>
                <TableCell>Source</TableCell>
                <TableCell>Attribute</TableCell>
                <TableCell>Active edges</TableCell>
                <TableCell>Deleted targets</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {(relationships.data ?? []).map((item) => (
                <TableRow
                  key={`${item.source_blueprint}-${item.attribute_code}`}
                >
                  <TableCell>{item.source_blueprint}</TableCell>
                  <TableCell>{item.attribute_code}</TableCell>
                  <TableCell>{item.active_edges}</TableCell>
                  <TableCell>{item.deleted_targets}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </AccordionDetails>
      </Accordion>
    </PageContainer>
  );
};
