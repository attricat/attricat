import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import {
  Accordion,
  AccordionDetails,
  AccordionSummary,
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
import { dataHealthQueryKeys } from './queryKeys';
import { DataHealthControls } from './DataHealthControls';
import { DataHealthSummaryCards } from './DataHealthSummaryCards';
import { dataHealthStaleTime, defaultStaleAfterDays } from './constants';
import type { DataHealthSearch } from './schemas';
import { formatDataHealthDate } from './dateFormat';
import { SectionError } from './SectionError';
import { formatBytes } from './dataHealthFormat';
import { ExtensionOutlet } from '../extensions/ExtensionOutlet';

export const DataHealthPage = ({ search }: { search: DataHealthSearch }) => {
  const { i18n, t } = useTranslation();
  const staleAfterDays = search.staleAfterDays ?? defaultStaleAfterDays;
  const navigate = useNavigate({ from: '/manage/data-health' });
  const queryClient = useQueryClient();
  const [showCompleteness, setShowCompleteness] = useState(false);
  const [showContexts, setShowContexts] = useState(false);
  const [showRelationships, setShowRelationships] = useState(false);
  const summary = useQuery({
    queryKey: dataHealthQueryKeys.summary(staleAfterDays),
    queryFn: () => getDataHealthSummary(staleAfterDays),
    staleTime: dataHealthStaleTime,
  });
  const blueprints = useQuery({
    queryKey: dataHealthQueryKeys.blueprints(staleAfterDays),
    queryFn: () => getDataHealthBlueprints(staleAfterDays),
    staleTime: dataHealthStaleTime,
  });
  const freshness = useQuery({
    queryKey: dataHealthQueryKeys.freshness(),
    queryFn: getDataHealthFreshness,
    staleTime: dataHealthStaleTime,
  });
  const storage = useQuery({
    queryKey: dataHealthQueryKeys.storage(),
    queryFn: getDataHealthStorage,
    staleTime: dataHealthStaleTime,
  });
  const completeness = useQuery({
    queryKey: dataHealthQueryKeys.completeness(),
    queryFn: getDataHealthCompleteness,
    enabled: showCompleteness,
    staleTime: dataHealthStaleTime,
  });
  const contexts = useQuery({
    queryKey: dataHealthQueryKeys.contexts(),
    queryFn: getDataHealthContexts,
    enabled: showContexts,
    staleTime: dataHealthStaleTime,
  });
  const relationships = useQuery({
    queryKey: dataHealthQueryKeys.relationships(),
    queryFn: getDataHealthRelationships,
    enabled: showRelationships,
    staleTime: dataHealthStaleTime,
  });
  const refresh = useMutation({
    mutationFn: refreshDataHealth,
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: dataHealthQueryKeys.all() }),
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
        description={t('dataHealth.description')}
        title={t('dataHealth.title')}
      />
      <SectionError error={summary.error} />
      {summary.data && (
        <>
          <DataHealthSummaryCards
            staleAfterDays={staleAfterDays}
            summary={summary.data}
          />
          <Box component="section" sx={{ mt: 2 }}>
            <ExtensionOutlet
              context={{ context_version: 1 }}
              outlet="data_health_card"
            />
          </Box>
        </>
      )}
      <Paper sx={{ mt: 4, p: 2 }}>
        <Typography variant="h5">{t('dataHealth.storage')}</Typography>
        <SectionError error={storage.error} />
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell>{t('dataHealth.table')}</TableCell>
              <TableCell align="right">{t('dataHealth.totalSize')}</TableCell>
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
          <Typography variant="h5">
            {t('dataHealth.blueprintHealth')}
          </Typography>
        </Box>
        <SectionError error={blueprints.error} />
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell>{t('dataHealth.blueprint')}</TableCell>
              <TableCell>{t('dataHealth.entities')}</TableCell>
              <TableCell>{t('dataHealth.outdated')}</TableCell>
              <TableCell>{t('dataHealth.stale')}</TableCell>
              <TableCell>{t('dataHealth.oldestUpdate')}</TableCell>
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
                        label={t('dataHealth.outdatedCount', {
                          count: blueprint.outdated_entities,
                        })}
                        size="small"
                      />
                    )}
                  </Stack>
                </TableCell>
                <TableCell>{blueprint.active_entities}</TableCell>
                <TableCell>{blueprint.outdated_entities}</TableCell>
                <TableCell>{blueprint.stale_entities}</TableCell>
                <TableCell>
                  {formatDataHealthDate(
                    blueprint.oldest_updated_at,
                    i18n.resolvedLanguage ?? i18n.language,
                    t('dataHealth.never'),
                  )}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </Paper>
      <Paper sx={{ mt: 4, p: 2 }}>
        <Typography variant="h5">
          {t('dataHealth.freshnessDistribution')}
        </Typography>
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
          <Typography variant="h5">
            {t('dataHealth.defaultCompleteness')}
          </Typography>
        </AccordionSummary>
        <AccordionDetails>
          <Typography color="text.secondary" sx={{ mb: 2 }}>
            {t('dataHealth.completenessDescription')}
          </Typography>
          <SectionError error={completeness.error} />
          <Table size="small">
            <TableHead>
              <TableRow>
                <TableCell>{t('dataHealth.blueprint')}</TableCell>
                <TableCell>{t('dataHealth.activeEntities')}</TableCell>
                <TableCell>{t('dataHealth.defaultComplete')}</TableCell>
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
                          label={t('dataHealth.outdatedCount', {
                            count: item.outdated_entities,
                          })}
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
          <Typography variant="h5">
            {t('dataHealth.contextCoverage')}
          </Typography>
        </AccordionSummary>
        <AccordionDetails>
          <SectionError error={contexts.error} />
          <Table size="small">
            <TableHead>
              <TableRow>
                <TableCell>{t('dataHealth.context')}</TableCell>
                <TableCell>
                  {t('dataHealth.entitiesWithDirectValues')}
                </TableCell>
                <TableCell>{t('dataHealth.currentDirectValues')}</TableCell>
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
          <Typography variant="h5">
            {t('dataHealth.relationshipIntegrity')}
          </Typography>
        </AccordionSummary>
        <AccordionDetails>
          <SectionError error={relationships.error} />
          <Table size="small">
            <TableHead>
              <TableRow>
                <TableCell>{t('dataHealth.source')}</TableCell>
                <TableCell>{t('dataHealth.attribute')}</TableCell>
                <TableCell>{t('dataHealth.activeEdges')}</TableCell>
                <TableCell>
                  {t('dataHealth.deletedRelationshipTargets')}
                </TableCell>
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
