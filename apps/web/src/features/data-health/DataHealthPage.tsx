import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Box } from '@mui/material';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { refreshDataHealth } from './api';
import { dataHealthOptions } from './queryOptions';
import { dataHealthQueryKeys } from './queryKeys';
import { DataHealthControls } from './DataHealthControls';
import { DataHealthSummaryCards } from './DataHealthSummaryCards';
import { defaultStaleAfterDays } from './constants';
import {
  BlueprintHealthSection,
  CompletenessSection,
  ContextCoverageSection,
  FreshnessSection,
  RelationshipIntegritySection,
  StorageSection,
} from './DataHealthSections';
import type { DataHealthSearch } from './schemas';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { ExtensionOutlet } from '../extensions/ExtensionOutlet';
import { DataHealthIcon } from '../../components/systemIcons';
import { ExportHealthCard } from '../extension-exports/ExportHealthCard';
import {
  extensionOutlets,
  supportedOutletContextVersion,
} from '../extensions/constants';

export const DataHealthPage = ({ search }: { search: DataHealthSearch }) => {
  const { t } = useTranslation();
  const staleAfterDays = search.staleAfterDays ?? defaultStaleAfterDays;
  const navigate = useNavigate({ from: '/manage/data-health' });
  const queryClient = useQueryClient();
  const [showCompleteness, setShowCompleteness] = useState(false);
  const [showContexts, setShowContexts] = useState(false);
  const [showRelationships, setShowRelationships] = useState(false);
  const summary = useQuery(dataHealthOptions.summary(staleAfterDays));
  const blueprints = useQuery(dataHealthOptions.blueprints(staleAfterDays));
  const freshness = useQuery(dataHealthOptions.freshness());
  const storage = useQuery(dataHealthOptions.storage());
  const completeness = useQuery({
    ...dataHealthOptions.completeness(),
    enabled: showCompleteness,
  });
  const contexts = useQuery({
    ...dataHealthOptions.contexts(),
    enabled: showContexts,
  });
  const relationships = useQuery({
    ...dataHealthOptions.relationships(),
    enabled: showRelationships,
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
      <ApiErrorAlert error={summary.error} />
      {summary.data && (
        <>
          <DataHealthSummaryCards
            staleAfterDays={staleAfterDays}
            summary={summary.data}
          />
          <ExportHealthCard />
          <Box component="section" sx={{ mt: 2 }}>
            <ExtensionOutlet
              context={{ context_version: supportedOutletContextVersion }}
              outlet={extensionOutlets.dataHealthCard}
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
