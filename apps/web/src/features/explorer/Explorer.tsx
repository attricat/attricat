import { Box, useMediaQuery, useTheme } from '@mui/material';
import { useNavigate } from '@tanstack/react-router';
import { useEffect, useMemo, useRef, useState } from 'react';
import { PageContainer } from '../../components/PageContainer';
import {
  SavedSearchesButton,
  ShareSearchButton,
} from '../saved-views/SavedSearchControls';
import type { SavedView } from '../saved-views/schemas';
import { ExplorerBrowseRail } from './ExplorerBrowseRail';
import { ExplorerRecordPanel } from './ExplorerRecordPanel';
import { ExplorerFilterBar } from './ExplorerFilterBar';
import { ExplorerLoadErrors } from './ExplorerLoadErrors';
import { ExplorerLoadingIndicator } from './ExplorerLoadingIndicator';
import { ExplorerOverview } from './ExplorerOverview';
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
import {
  clearLastBlueprint,
  getLastBlueprint,
  setLastBlueprint,
} from './lastBlueprint';
import type { ExplorerSearch } from './search';
import type { OpenRecordPanel } from './ExplorerTableCells';
import { useBrowseBy } from './useBrowseBy';
import { useExplorerData } from './useExplorerData';
import { useSavedSearchActions } from '../saved-views/useSavedSearchActions';
import { useExplorerSearchActions } from './useExplorerSearchActions';
import { lexiconText } from '../lexicon/lexicon';

type Props = {
  /** The result open in the record panel on wide screens. */
  panelRecordId?: string;
  search: ExplorerSearch;
  savedView?: SavedView;
};

export const Explorer = ({
  panelRecordId,
  search: urlSearch,
  savedView,
}: Props) => {
  const queryInputRef = useRef<HTMLInputElement>(null);
  const theme = useTheme();
  const isWideDesktop = useMediaQuery(theme.breakpoints.up('lg'));
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
  const explorerView = selectedBlueprint.data?.blueprint.views.explorer;
  const browseBy = useBrowseBy({
    blueprintCode: search.blueprint,
    blueprintDefault:
      explorerView?.type === 'explorer' ? explorerView.browse_by : undefined,
    blueprintId: currentBlueprint?.id,
    onChange: actions.changeBrowseBy,
    relationshipFields,
    requested: search.browseBy,
  });
  const facets = selectedRelationshipFacets(
    relationshipFacetSources(relationshipFields, search),
    search,
  );
  const blueprintName =
    (selectedBlueprint.data &&
      lexiconText(selectedBlueprint.data.blueprint.name)) ??
    search.blueprint;
  // Only a first load shows the spinner: a new blueprint, or results still
  // waiting to run. Background refetches (such as on window focus) and
  // search changes keep the table in place and show its own progress bar.
  const loading =
    Boolean(search.blueprint) &&
    !blueprintMissing &&
    (blueprints.isLoading ||
      selectedBlueprint.isLoading ||
      (canSearch && results.isPending));
  // Narrower screens open results on their own page.
  const panelOpen = isWideDesktop && panelRecordId !== undefined;
  const showInPanel = (record: string | undefined) =>
    void navigate({
      to: '/',
      search: (previous) => ({ ...previous, record }),
      replace: true,
    });
  // Clicking the record already in the panel again opens its full page.
  const openPanel: OpenRecordPanel = (recordId, opener) => {
    if (panelOpen && recordId === panelRecordId) {
      void navigate({ to: '/records/$recordId', params: { recordId } });
      return;
    }
    panelOpenerRef.current = opener;
    showInPanel(recordId);
  };
  const closePanel = () => {
    showInPanel(undefined);
    if (panelOpenerRef.current?.isConnected) panelOpenerRef.current.focus();
  };

  return (
    <Box sx={{ display: 'flex', minHeight: '100dvh' }}>
      <Box sx={{ flexGrow: 1, minWidth: 0 }}>
        <PageContainer>
          <ExplorerPageHeader
            blueprint={blueprintMissing ? undefined : search.blueprint}
            blueprintName={blueprintMissing ? undefined : blueprintName}
            blueprintRecord={
              blueprintMissing ? undefined : selectedBlueprint.data?.blueprint
            }
            locked={Boolean(search.locked)}
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
            lockedBlueprint={Boolean(search.locked)}
            onBlueprintChange={(value) => {
              // Otherwise the remembered blueprint is restored at once.
              if (!value) clearLastBlueprint();
              actions.selectBlueprint(value || undefined);
            }}
            contextCode={contextCode}
            contexts={contexts.data}
            onContextChange={actions.changeContext}
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
          {search.blueprint && !blueprintMissing && (
            <ExplorerFilterBar
              attributes={selectedBlueprint.data?.attributes ?? []}
              blueprint={search.blueprint}
              blueprints={blueprints.data}
              emptyFocusTarget={queryInputRef}
              facets={facets}
              filterRequest={filterRequest}
              filters={search.attributeFilters ?? []}
              key={search.blueprint}
              onAdd={actions.addAttributeFilter}
              onRemove={actions.removeAttributeFilter}
              onUpdate={actions.updateAttributeFilter}
              onUpdateFacet={actions.updateFacet}
              pathAttributes={selectedBlueprint.data?.table_path_attributes}
              relationshipAttributes={relationshipFields}
            />
          )}
          {!search.blueprint && (
            <ExplorerOverview
              blueprints={blueprints.data}
              contextCode={contextCode}
              contextCodes={data.contextCodes}
              contextId={data.contextId}
              onExplore={actions.exploreBlueprint}
              onOpenPanel={isWideDesktop ? openPanel : undefined}
              panelRecordId={panelOpen ? panelRecordId : undefined}
              publicationSortAvailable={data.publicationSortAvailable}
              search={search}
            />
          )}
          {loading && <ExplorerLoadingIndicator />}
          <Box sx={{ alignItems: 'flex-start', display: 'flex', gap: 2 }}>
            {isWideDesktop && search.blueprint && !blueprintMissing && (
              <ExplorerBrowseRail
                blueprint={search.blueprint}
                blueprintName={blueprintName ?? search.blueprint}
                contextId={data.contextId}
                defaultField={browseBy.defaultField}
                field={browseBy.field}
                fields={browseBy.fields}
                onFieldChange={browseBy.change}
                onSelect={(selectedIds) => {
                  const field = browseBy.field;
                  if (!field) return;
                  actions.updateFacet(field.attribute.code, {
                    hierarchyField: field.hierarchyField,
                    ownTree: field.ownTree,
                    selectedIds,
                    targetBlueprint: field.attribute.target_blueprint_code,
                  });
                }}
                query={search.query}
                selectedIds={
                  search.relationshipFacets?.find(
                    (facet) =>
                      facet.field === browseBy.field?.attribute.code &&
                      Boolean(facet.ownTree) ===
                        Boolean(browseBy.field.ownTree),
                  )?.selectedIds ?? []
                }
                version={data.effectiveVersion}
              />
            )}
            <Box sx={{ flexGrow: 1, minWidth: 0 }}>
              {/* Kept results of the last blueprint must not show under the
                  overview. */}
              {search.blueprint && (
                <ExplorerResults
                  data={data}
                  onFilterCell={requestFilter}
                  onOpenPanel={isWideDesktop ? openPanel : undefined}
                  panelRecordId={panelOpen ? panelRecordId : undefined}
                  onShowAllVersions={actions.showAllVersions}
                  onSortChange={actions.toggleSort}
                  search={search}
                />
              )}
            </Box>
          </Box>
        </PageContainer>
      </Box>
      {panelOpen && (
        <ExplorerRecordPanel
          contextId={data.contextId}
          recordId={panelRecordId}
          key={panelRecordId}
          onClose={closePanel}
          onOpenRecord={showInPanel}
        />
      )}
    </Box>
  );
};
