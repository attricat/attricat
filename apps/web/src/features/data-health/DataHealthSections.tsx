import { ChevronDownIcon } from 'lucide-react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Accordion,
  AccordionDetails,
  AccordionSummary,
  Box,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import type { UseQueryResult } from '@tanstack/react-query';
import { BlueprintLabel } from './BlueprintLabel';
import { formatBytes } from './dataHealthFormat';
import { Timestamp } from '../../time/Timestamp';
import { SectionError } from './SectionError';
import type {
  BlueprintHealth,
  CompletenessHealth,
  ContextHealth,
  FreshnessBand,
  RelationshipHealth,
  StorageHealth,
} from './schemas';

type SectionProps<T> = { query: UseQueryResult<T[]> };

export const StorageSection = ({ query }: SectionProps<StorageHealth>) => {
  const { t } = useTranslation();

  return (
    <Paper sx={{ mt: 4, p: 2 }}>
      <Typography variant="h5">{t('dataHealth.storage')}</Typography>
      <SectionError error={query.error} />
      <Table size="small">
        <TableHead>
          <TableRow>
            <TableCell>{t('dataHealth.table')}</TableCell>
            <TableCell align="right">{t('dataHealth.totalSize')}</TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {(query.data ?? []).map((item) => (
            <TableRow key={item.table}>
              <TableCell>{item.table}</TableCell>
              <TableCell align="right">{formatBytes(item.bytes)}</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </Paper>
  );
};

export const BlueprintHealthSection = ({
  query,
}: SectionProps<BlueprintHealth>) => {
  const { t } = useTranslation();

  return (
    <Paper sx={{ mt: 4, overflowX: 'auto' }}>
      <Box sx={{ p: 2 }}>
        <Typography variant="h5">{t('dataHealth.blueprintHealth')}</Typography>
      </Box>
      <SectionError error={query.error} />
      <Table size="small">
        <TableHead>
          <TableRow>
            <TableCell>{t('dataHealth.blueprint')}</TableCell>
            <TableCell>{t('dataHealth.records')}</TableCell>
            <TableCell>{t('dataHealth.outdated')}</TableCell>
            <TableCell>{t('dataHealth.stale')}</TableCell>
            <TableCell>{t('dataHealth.oldestUpdate')}</TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {(query.data ?? []).map((blueprint) => (
            <TableRow key={blueprint.code}>
              <TableCell>
                <BlueprintLabel
                  code={blueprint.code}
                  currentVersion={blueprint.current_version}
                  name={blueprint.name}
                  outdatedRecords={blueprint.outdated_records}
                />
              </TableCell>
              <TableCell>{blueprint.active_records}</TableCell>
              <TableCell>{blueprint.outdated_records}</TableCell>
              <TableCell>{blueprint.stale_records}</TableCell>
              <TableCell>
                <Timestamp
                  fallback={t('dataHealth.never')}
                  style="date"
                  value={blueprint.oldest_updated_at}
                />
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </Paper>
  );
};

export const FreshnessSection = ({ query }: SectionProps<FreshnessBand>) => {
  const { t } = useTranslation();

  return (
    <Paper sx={{ mt: 4, p: 2 }}>
      <Typography variant="h5">
        {t('dataHealth.freshnessDistribution')}
      </Typography>
      <SectionError error={query.error} />
      <Stack direction="row" spacing={3} sx={{ mt: 2 }}>
        {(query.data ?? []).map((band) => (
          <Box key={band.label}>
            <Typography variant="h5">{band.records}</Typography>
            <Typography color="text.secondary" variant="body2">
              {band.label}
            </Typography>
          </Box>
        ))}
      </Stack>
    </Paper>
  );
};

const LazyAccordion = ({
  children,
  onExpandedChange,
  sx,
  title,
}: {
  children: ReactNode;
  onExpandedChange: (expanded: boolean) => void;
  sx?: { mt: number };
  title: string;
}) => (
  <Accordion onChange={(_, expanded) => onExpandedChange(expanded)} sx={sx}>
    <AccordionSummary expandIcon={<ChevronDownIcon />}>
      <Typography variant="h5">{title}</Typography>
    </AccordionSummary>
    <AccordionDetails>{children}</AccordionDetails>
  </Accordion>
);

type AccordionProps<T> = SectionProps<T> & {
  onExpandedChange: (expanded: boolean) => void;
};

export const CompletenessSection = ({
  onExpandedChange,
  query,
}: AccordionProps<CompletenessHealth>) => {
  const { t } = useTranslation();

  return (
    <LazyAccordion
      onExpandedChange={onExpandedChange}
      sx={{ mt: 4 }}
      title={t('dataHealth.defaultCompleteness')}
    >
      <Typography color="text.secondary" sx={{ mb: 2 }}>
        {t('dataHealth.completenessDescription')}
      </Typography>
      <SectionError error={query.error} />
      <Table size="small">
        <TableHead>
          <TableRow>
            <TableCell>{t('dataHealth.blueprint')}</TableCell>
            <TableCell>{t('dataHealth.activeRecords')}</TableCell>
            <TableCell>{t('dataHealth.defaultComplete')}</TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {(query.data ?? []).map((item) => (
            <TableRow key={item.code}>
              <TableCell>
                <BlueprintLabel
                  code={item.code}
                  currentVersion={item.current_version}
                  name={item.name}
                  outdatedRecords={item.outdated_records}
                />
              </TableCell>
              <TableCell>{item.active_records}</TableCell>
              <TableCell>{item.default_complete_records}</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </LazyAccordion>
  );
};

export const ContextCoverageSection = ({
  onExpandedChange,
  query,
}: AccordionProps<ContextHealth>) => {
  const { t } = useTranslation();

  return (
    <LazyAccordion
      onExpandedChange={onExpandedChange}
      title={t('dataHealth.contextCoverage')}
    >
      <SectionError error={query.error} />
      <Table size="small">
        <TableHead>
          <TableRow>
            <TableCell>{t('dataHealth.context')}</TableCell>
            <TableCell>{t('dataHealth.recordsWithDirectValues')}</TableCell>
            <TableCell>{t('dataHealth.currentDirectValues')}</TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {(query.data ?? []).map((item) => (
            <TableRow key={item.code}>
              <TableCell>{item.code}</TableCell>
              <TableCell>{item.direct_records}</TableCell>
              <TableCell>{item.direct_values}</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </LazyAccordion>
  );
};

export const RelationshipIntegritySection = ({
  onExpandedChange,
  query,
}: AccordionProps<RelationshipHealth>) => {
  const { t } = useTranslation();

  return (
    <LazyAccordion
      onExpandedChange={onExpandedChange}
      title={t('dataHealth.relationshipIntegrity')}
    >
      <SectionError error={query.error} />
      <Table size="small">
        <TableHead>
          <TableRow>
            <TableCell>{t('dataHealth.source')}</TableCell>
            <TableCell>{t('dataHealth.attribute')}</TableCell>
            <TableCell>{t('dataHealth.activeEdges')}</TableCell>
            <TableCell>{t('dataHealth.deletedRelationshipTargets')}</TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {(query.data ?? []).map((item) => (
            <TableRow key={`${item.source_blueprint}-${item.attribute_code}`}>
              <TableCell>{item.source_blueprint}</TableCell>
              <TableCell>{item.attribute_code}</TableCell>
              <TableCell>{item.active_edges}</TableCell>
              <TableCell>{item.deleted_targets}</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </LazyAccordion>
  );
};
