import { Box, Typography, useMediaQuery, useTheme } from '@mui/material';
import { useNavigate } from '@tanstack/react-router';
import { useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';
import { useMobileExplorePanelTarget } from '../../components/mobileNavigationPanelContext';
import { PageContainer } from '../../components/PageContainer';
import {
  SavedSearchesButton,
  ShareSearchButton,
} from '../saved-views/SavedSearchControls';
import type { SavedView } from '../saved-views/schemas';
import { ActiveExplorerFilters } from './ActiveExplorerFilters';
import { facetSidebarWidth } from './constants';
import { ExplorerEntityPanel } from './ExplorerEntityPanel';
import { ExplorerFacetSidebar } from './ExplorerFacetSidebar';
import { ExplorerLoadErrors } from './ExplorerLoadErrors';
import { ExplorerLoadingIndicator } from './ExplorerLoadingIndicator';
import { ExplorerPageHeader } from './ExplorerPageHeader';
import { ExplorerResults } from './ExplorerResults';
import { ExplorerSearchForm } from './ExplorerSearchForm';
import type {
  AttributeFilterDraft,
  AttributeFilterRequest,
} from './attributeFilterValues';
import {
  relationshipFacetSources,
  selectedRelationshipFacets,
} from './explorerSearchState';
import { getLastBlueprint, setLastBlueprint } from './lastBlueprint';
import type { ExplorerSearch } from './search';
import type { OpenEntityPanel } from './ExplorerTableCells';
import { useExplorerData } from './useExplorerData';
import { useSavedSearchActions } from '../saved-views/useSavedSearchActions';
import { useExplorerSearchActions } from './useExplorerSearchActions';
import { lexiconText } from '../lexicon/lexicon';

type Props = {
  /** The result open in the entity panel on wide screens. */
  panelEntityId?: string;
  search: ExplorerSearch;
  savedView?: SavedView;
};

export const Explorer = ({
  panelEntityId,
  search: urlSearch,
  savedView,
}: Props) => {
  const { t } = useTranslation();
  const queryInputRef = useRef<HTMLInputElement>(null);
  const theme = useTheme();
  const isWideDesktop = useMediaQuery(theme.breakpoints.up('lg'));
  const mobileExplorePanelTarget = useMobileExplorePanelTarget();
  const navigate = useNavigate({ from: '/' });
  const panelOpenerRef = useRef<HTMLElement | null>(null);
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

  const [filterRequest, setFilterRequest] = useState<AttributeFilterRequest>();
  const requestFilter = (draft: AttributeFilterDraft) =>
    setFilterRequest((current) => ({ id: (current?.id ?? 0) + 1, draft }));

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
  const savedSearchActions = useSavedSearchActions({
    savedView,
    search,
    userId: session.data?.user_id,
  });
  const facets = selectedRelationshipFacets(
    relationshipFacetSources(relationshipFields, search),
    search,
  );
  const lockedBlueprintName =
    (selectedBlueprint.data &&
      lexiconText(selectedBlueprint.data.blueprint.name)) ??
    search.blueprint;
  const loading =
    Boolean(search.blueprint) &&
    !blueprintMissing &&
    (blueprints.isFetching ||
      selectedBlueprint.isFetching ||
      (canSearch && results.isPending && !results.data));
  // Narrower screens open results on their own page.
  const panelOpen = isWideDesktop && panelEntityId !== undefined;
  const showInPanel = (entity: string | undefined) =>
    void navigate({
      to: '/',
      search: (previous) => ({ ...previous, entity }),
      replace: true,
    });
  const openPanel: OpenEntityPanel = (entityId, opener) => {
    panelOpenerRef.current = opener;
    showInPanel(entityId);
  };
  const closePanel = () => {
    showInPanel(undefined);
    if (panelOpenerRef.current?.isConnected) panelOpenerRef.current.focus();
  };
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
        {/* Always mounted (hidden below lg), so it owns cell filter requests. */}
        <ExplorerFacetSidebar
          {...facetSidebarProps}
          filterRequest={filterRequest}
          fullHeight
        />
      </Box>
      <Box sx={{ flexGrow: 1, minWidth: 0 }}>
        <PageContainer>
          <ExplorerPageHeader
            blueprint={blueprintMissing ? undefined : search.blueprint}
            locked={Boolean(search.locked)}
            lockedBlueprintName={lockedBlueprintName}
          />
          <ExplorerLoadErrors
            blueprintMissing={blueprintMissing}
            blueprints={blueprints}
            selectedBlueprint={selectedBlueprint}
          />
          <ExplorerSearchForm
            blueprints={blueprints.data ?? []}
            blueprintSchema={selectedBlueprint.data}
            currentVersion={currentBlueprint?.version}
            revisions={revisions.data}
            revisionsError={revisions.error?.message}
            revisionsLoading={revisions.isPending}
            onRetryRevisions={() => void revisions.refetch()}
            lockedBlueprint
            onSubmit={actions.submit}
            search={search}
            startActions={
              <SavedSearchesButton
                actions={savedSearchActions}
                search={search}
              />
            }
            endActions={
              <ShareSearchButton actions={savedSearchActions} search={search} />
            }
            queryInputRef={queryInputRef}
          />
          <ActiveExplorerFilters
            attributes={selectedBlueprint.data?.attributes}
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
            emptyFocusTarget={queryInputRef}
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
            onFilterCell={requestFilter}
            onOpenPanel={isWideDesktop ? openPanel : undefined}
            panelEntityId={panelOpen ? panelEntityId : undefined}
            onShowAllVersions={actions.showAllVersions}
            onSortChange={actions.toggleSort}
            search={search}
          />
        </PageContainer>
      </Box>
      {panelOpen && (
        <ExplorerEntityPanel
          contextId={data.contextId}
          entityId={panelEntityId}
          key={panelEntityId}
          onClose={closePanel}
          onOpenEntity={showInPanel}
        />
      )}
      {mobileExplorePanelTarget &&
        !isWideDesktop &&
        createPortal(
          <ExplorerFacetSidebar {...facetSidebarProps} />,
          mobileExplorePanelTarget,
        )}
    </Box>
  );
};
