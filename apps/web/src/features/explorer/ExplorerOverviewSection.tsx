import { Alert, Box, Button, Stack, Typography } from '@mui/material';
import { ArrowRightIcon } from 'lucide-react';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type {
  Blueprint,
  RecordItem,
  RecordSearchResponse,
} from '../records/api';
import { SaveSelectionAsSearchDialog } from '../saved-views/SaveSelectionAsSearchDialog';
import { lexiconText } from '../lexicon/lexicon';
import { ExplorerResultsTable } from './ExplorerResultsTable';
import { toggledSort } from './explorerSearchState';
import type { OpenRecordPanel } from './ExplorerTableCells';
import type { ExplorerSort } from './search';
import { useExplorerSelection } from './useExplorerSelection';

type Props = {
  blueprint: Blueprint;
  contextCode: string;
  /** The selected context followed by its ancestors. */
  contextCodes: readonly string[];
  contextId: string | undefined;
  error: Error | null;
  isFetching: boolean;
  onExplore: (blueprint: string, sort?: ExplorerSort) => void;
  onOpenPanel?: OpenRecordPanel;
  page: RecordSearchResponse | undefined;
  publicationSortAvailable: boolean;
  query: string | undefined;
};

/** A preview of one blueprint's first matching records. */
export const ExplorerOverviewSection = ({
  blueprint,
  contextCode,
  contextCodes,
  contextId,
  error,
  isFetching,
  onExplore,
  onOpenPanel,
  page,
  publicationSortAvailable,
  query,
}: Props) => {
  const { t } = useTranslation();
  const headingId = useId();
  const name = lexiconText(blueprint.name);
  const items = page?.items ?? [];
  const selection = useExplorerSelection(blueprint.code, items);
  const [savedSearchSelection, setSavedSearchSelection] = useState<
    RecordItem[] | null
  >(null);
  const singleVersionScope =
    page?.result_version_scope.kind === 'single'
      ? page.result_version_scope
      : undefined;

  return (
    <Box aria-labelledby={headingId} component="section" sx={{ mt: 3 }}>
      <Stack
        direction="row"
        sx={{ alignItems: 'center', justifyContent: 'space-between', mb: 1 }}
      >
        <Typography component="h2" id={headingId} variant="h6">
          {name}
        </Typography>
        <Button
          aria-label={t('explorer.showAllOfBlueprint', { blueprint: name })}
          endIcon={<ArrowRightIcon aria-hidden />}
          onClick={() => onExplore(blueprint.code)}
        >
          {t('explorer.showAll')}
        </Button>
      </Stack>
      {error ? (
        <Alert severity="error">{error.message}</Alert>
      ) : (
        page && (
          <ExplorerResultsTable
            blueprint={page.blueprint}
            contextCodes={contextCodes}
            fitRows
            hasNextPage={false}
            isFetching={isFetching}
            isFetchingNextPage={false}
            items={items}
            onLoadMore={() => {}}
            onOpenPanel={onOpenPanel}
            onSaveSelectionAsSearch={setSavedSearchSelection}
            onSortChange={(field) =>
              onExplore(blueprint.code, toggledSort(undefined, field))
            }
            publicationContextCode={contextCode}
            publicationContextId={contextId}
            publicationSortAvailable={publicationSortAvailable}
            selection={selection}
            showExplorerActions={
              singleVersionScope?.version === page.blueprint.blueprint.version
            }
            totalCount={page.total_count ?? null}
            totalCountCapped={page.total_count_capped ?? false}
          />
        )
      )}
      {savedSearchSelection && (
        <SaveSelectionAsSearchDialog
          records={savedSearchSelection}
          onClose={() => setSavedSearchSelection(null)}
          onSuccess={() => {
            setSavedSearchSelection(null);
            selection.exitSelectionMode();
          }}
          search={{ blueprint: blueprint.code, context: contextCode, query }}
        />
      )}
    </Box>
  );
};
