import { Box, Typography, useMediaQuery, useTheme } from '@mui/material';
import { useEffect, useMemo } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';
import { useMobileExplorePanelTarget } from '../../components/mobileNavigationPanelContext';
import { PageContainer } from '../../components/PageContainer';
import { SavedSearchActions } from '../saved-views/SavedSearchActions';
import type { SavedView } from '../saved-views/schemas';
import { ActiveExplorerFilters } from './ActiveExplorerFilters';
import { facetSidebarWidth } from './constants';
import { ExplorerFacetSidebar } from './ExplorerFacetSidebar';
import { ExplorerLoadErrors } from './ExplorerLoadErrors';
import { ExplorerLoadingIndicator } from './ExplorerLoadingIndicator';
import { ExplorerPageHeader } from './ExplorerPageHeader';
import { ExplorerResults } from './ExplorerResults';
import { ExplorerSearchForm } from './ExplorerSearchForm';
import {
  relationshipFacetSources,
  selectedRelationshipFacets,
} from './explorerSearchState';
import { getLastBlueprint, setLastBlueprint } from './lastBlueprint';
import type { ExplorerSearch } from './search';
import { useExplorerData } from './useExplorerData';
import { useExplorerSearchActions } from './useExplorerSearchActions';

type Props = {
  search: ExplorerSearch;
  savedView?: SavedView;
};

export const Explorer = ({ search: urlSearch, savedView }: Props) => {
  const { t } = useTranslation();
  const theme = useTheme();
  const isWideDesktop = useMediaQuery(theme.breakpoints.up('lg'));
  const mobileExplorePanelTarget = useMobileExplorePanelTarget();
  const search = useMemo(
    () => ({
      ...urlSearch,
      blueprint: urlSearch.blueprint ?? getLastBlueprint(),
    }),
    [urlSearch],
  );

  useEffect(() => {
    if (urlSearch.blueprint) setLastBlueprint(urlSearch.blueprint);
  }, [urlSearch.blueprint]);

  const data = useExplorerData(search);
  const {
    blueprintMissing,
    blueprints,
    canSearch,
    contextCode,
    contexts,
    currentBlueprint,
    relationshipFields,
    relationshipFilters,
    results,
    revisions,
    selectedBlueprint,
    session,
  } = data;
  const actions = useExplorerSearchActions(search, {
    currentVersion: currentBlueprint?.version,
    relationshipFilters,
  });
  const facets = selectedRelationshipFacets(
    relationshipFacetSources(relationshipFields, search),
    search,
  );
  const lockedBlueprintName =
    selectedBlueprint.data?.blueprint.name ?? search.blueprint;
  const loading =
    Boolean(search.blueprint) &&
    !blueprintMissing &&
    (blueprints.isFetching ||
      selectedBlueprint.isFetching ||
      (canSearch && results.isPending && !results.data));
  const facetSidebarProps = {
    attributeFilters: search.attributeFilters ?? [],
    attributes: selectedBlueprint.data?.attributes ?? [],
    blueprint: search.blueprint ?? '',
    blueprints: blueprints.data ?? [],
    contextCode,
    contexts: contexts.data ?? [],
    facets,
    onAddAttributeFilter: actions.addAttributeFilter,
    relationshipAttributes: relationshipFields,
    onBlueprintChange: search.locked ? undefined : actions.selectBlueprint,
    onContextChange: actions.changeContext,
    onRemoveAttributeFilter: actions.removeAttributeFilter,
    onUpdate: actions.updateFacet,
    onUpdateAttributeFilter: actions.updateAttributeFilter,
    pathAttributes: selectedBlueprint.data?.table_path_attributes,
  };

  return (
    <Box sx={{ display: 'flex', minHeight: '100dvh' }}>
      <Box
        sx={{
          display: { xs: 'none', lg: 'block' },
          flexShrink: 0,
          width: facetSidebarWidth,
        }}
      >
        <ExplorerFacetSidebar {...facetSidebarProps} fullHeight />
      </Box>
      <Box sx={{ flexGrow: 1, minWidth: 0 }}>
        <PageContainer>
          <ExplorerPageHeader
            blueprint={search.blueprint}
            locked={Boolean(search.locked)}
            lockedBlueprintName={lockedBlueprintName}
          />
          <ExplorerLoadErrors
            blueprintMissing={blueprintMissing}
            blueprints={blueprints}
            selectedBlueprint={selectedBlueprint}
          />
          <SavedSearchActions
            search={search}
            savedView={savedView}
            userId={session.data?.user_id}
          />
          <ExplorerSearchForm
            blueprints={blueprints.data ?? []}
            currentVersion={currentBlueprint?.version}
            revisions={revisions.data}
            revisionsError={revisions.error?.message}
            revisionsLoading={revisions.isPending}
            onRetryRevisions={() => void revisions.refetch()}
            lockedBlueprint
            onSubmit={actions.submit}
            search={search}
          />
          <ActiveExplorerFilters
            filters={[
              ...(search.attributeFilters ?? []).map((filter, index) => ({
                filter,
                index,
                kind: 'attribute' as const,
              })),
              ...facets.map((facet) => ({
                field: facet.sourceRelationship.code,
                kind: 'relationship' as const,
                selectedCount: facet.selectedIds.length,
              })),
            ]}
            onRemoveAttribute={actions.removeAttributeFilter}
            onRemoveRelationship={(field) =>
              actions.updateFacet(field, { selectedIds: [] })
            }
          />
          {!search.blueprint && (
            <Typography sx={{ py: 3 }}>{t('explorer.start')}</Typography>
          )}
          {loading && <ExplorerLoadingIndicator />}
          <ExplorerResults
            data={data}
            onShowAllVersions={actions.showAllVersions}
            onSortChange={actions.toggleSort}
            search={search}
          />
        </PageContainer>
      </Box>
      {mobileExplorePanelTarget &&
        !isWideDesktop &&
        createPortal(
          <ExplorerFacetSidebar {...facetSidebarProps} />,
          mobileExplorePanelTarget,
        )}
    </Box>
  );
};
