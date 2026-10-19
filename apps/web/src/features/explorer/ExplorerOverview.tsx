import { Alert, Paper } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { EmptyState } from '../../components/EmptyState';
import { BlueprintIcon, RecordIcon } from '../../components/systemIcons';
import type { Blueprint } from '../records/api';
import { ExplorerLoadingIndicator } from './ExplorerLoadingIndicator';
import { ExplorerOverviewSection } from './ExplorerOverviewSection';
import type { OpenRecordPanel } from './ExplorerTableCells';
import type { ExplorerSearch, ExplorerSort } from './search';
import { isRejectedQuery, useExplorerOverview } from './useExplorerOverview';

type Props = {
  /** Undefined while the blueprints load. */
  blueprints: Blueprint[] | undefined;
  contextCode: string;
  /** The selected context followed by its ancestors. */
  contextCodes: readonly string[];
  contextId: string | undefined;
  onExplore: (blueprint: string, sort?: ExplorerSort) => void;
  onOpenPanel?: OpenRecordPanel;
  panelRecordId?: string;
  publicationSortAvailable: boolean;
  search: ExplorerSearch;
};

/** One section per blueprint with matching records, shown when none is chosen. */
export const ExplorerOverview = ({
  blueprints,
  contextCode,
  contextCodes,
  contextId,
  onExplore,
  onOpenPanel,
  panelRecordId,
  publicationSortAvailable,
  search,
}: Props) => {
  const { t } = useTranslation();
  const results = useExplorerOverview(blueprints ?? [], {
    contextCode,
    query: search.query ?? '',
  });

  if (!blueprints) return <ExplorerLoadingIndicator />;
  if (blueprints.length === 0)
    return (
      <Paper sx={{ mt: 3 }}>
        <EmptyState icon={BlueprintIcon} title={t('explorer.noBlueprints')} />
      </Paper>
    );

  const sections = blueprints.flatMap((blueprint, index) => {
    const result = results[index];
    const visible = result.isError
      ? !isRejectedQuery(result.error)
      : Boolean(result.data?.items.length);
    return visible ? [{ blueprint, result }] : [];
  });

  if (sections.length === 0) {
    if (results.some((result) => result.isPending))
      return <ExplorerLoadingIndicator />;
    const rejection = results.every((result) => isRejectedQuery(result.error))
      ? results[0]?.error
      : undefined;
    return rejection ? (
      <Alert severity="error" sx={{ mt: 3 }}>
        {rejection.message}
      </Alert>
    ) : (
      <Paper sx={{ mt: 3 }}>
        <EmptyState icon={RecordIcon} title={t('explorer.noMatchingRecords')} />
      </Paper>
    );
  }

  return sections.map(({ blueprint, result }) => (
    <ExplorerOverviewSection
      blueprint={blueprint}
      contextCode={contextCode}
      contextCodes={contextCodes}
      contextId={contextId}
      error={result.error}
      isFetching={result.isFetching}
      key={blueprint.code}
      onExplore={onExplore}
      onOpenPanel={onOpenPanel}
      page={result.data}
      panelRecordId={panelRecordId}
      publicationSortAvailable={publicationSortAvailable}
      query={search.query}
    />
  ));
};
