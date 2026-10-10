import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Box } from '@mui/material';
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
import {
  dataHealthExtensionContextVersion,
  dataHealthExtensionOutlet,
  dataHealthStaleTime,
  defaultStaleAfterDays,
} from './constants';
import {
  BlueprintHealthSection,
  CompletenessSection,
  ContextCoverageSection,
  FreshnessSection,
  RelationshipIntegritySection,
  StorageSection,
} from './DataHealthSections';
import type { DataHealthSearch } from './schemas';
import { SectionError } from './SectionError';
import { ExtensionOutlet } from '../extensions/ExtensionOutlet';
import { DataHealthIcon } from '../../components/systemIcons';

export const DataHealthPage = ({ search }: { search: DataHealthSearch }) => {
  const { t } = useTranslation();
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
        icon={DataHealthIcon}
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
              context={{ context_version: dataHealthExtensionContextVersion }}
              outlet={dataHealthExtensionOutlet}
            />
          </Box>
        </>
      )}
      <StorageSection query={storage} />
      <BlueprintHealthSection query={blueprints} />
      <FreshnessSection query={freshness} />
      <CompletenessSection
        onExpandedChange={setShowCompleteness}
        query={completeness}
      />
      <ContextCoverageSection
        onExpandedChange={setShowContexts}
        query={contexts}
      />
      <RelationshipIntegritySection
        onExpandedChange={setShowRelationships}
        query={relationships}
      />
    </PageContainer>
  );
};
