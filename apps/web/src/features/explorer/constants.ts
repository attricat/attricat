import {
  attributeFilterOperators,
  NEEDS_ATTENTION_SYSTEM_TAG,
  SYSTEM_TAG_SORT_PREFIX,
} from '../records/constants';
import type { ApiErrorCode } from '../../api/errors';

// Search and filter limits.
export const maximumAttributeFilters = 20;
/**
 * Most system tags one search filters by, as the API's `MAX_SYSTEM_TAG_FILTERS`;
 * saved searches store as many.
 */
export const maximumSystemTagFilters = 20;
/** Delay before a typed tag prefix asks the server for matching tags. */
export const systemTagSearchDebounceMs = 250;
/** How long listed tag counts stay fresh while the filter is reopened. */
export const systemTagFacetStaleTimeMs = 30_000;
export const maximumAgentSelection = 50;
export const maximumAgentInstructionsLength = 1_000;
export const maximumAgentConversationBlueprintNameLength = 120;
export const maximumAgentRecordLabelLength = 60;

// Storage keys.
export const lastBlueprintStorageKey = 'attricat.explorer.last-blueprint';
export const columnPreferencesStorageKeyPrefix =
  'attricat.explorer.column-preferences.';
export const browseByStorageKeyPrefix = 'attricat.explorer.browse-by.';

// Attribute visibility scope used to hide attributes from the Explorer.
export const explorerVisibilityScope = 'explorer';

// Relationship paths such as `brand.country` join attribute codes with a dot.
export const relationshipPathSeparator = '.';
/** Joins the hop labels of a relationship path, as in "Model → Brand". */
export const relationshipPathLabelSeparator = ' → ';
/** Most relationship hops a Browse rail path follows. */
export const maximumBrowsePathHops = 3;
/** Most relationship paths the Browse rail offers after the direct fields. */
export const maximumBrowsePaths = 15;

export const defaultAttributeFilterOperator = attributeFilterOperators.eq;
/** Cell shortcuts always filter by equality rather than guessing an operator. */
export const cellFilterOperator = attributeFilterOperators.eq;

export const versionScopes = {
  all: 'all',
  current: 'current',
} as const;

export const sortDirections = {
  ascending: 'asc',
  descending: 'desc',
} as const;

export const explorerSortFields = {
  blueprintVersion: 'blueprint_version',
  /** Records that need attention, apart from the rest. */
  dataQuality: `${SYSTEM_TAG_SORT_PREFIX}${NEEDS_ATTENTION_SYSTEM_TAG}`,
  publicationStatus: 'publication_status',
} as const;

export const relationshipPathSortErrorCode =
  'relationship_path_sort_requires_single_result_version' satisfies ApiErrorCode;

export const explorerColumnIds = {
  actions: 'actions',
  dataQuality: 'data-quality',
  display: 'display',
  id: 'id',
  publication: 'publication',
  schema: 'schema',
  select: 'select',
  systemTags: 'systemTags',
} as const;

export const builtInConfigurableColumnIds: string[] = [
  explorerColumnIds.id,
  explorerColumnIds.display,
  explorerColumnIds.publication,
  explorerColumnIds.schema,
  explorerColumnIds.systemTags,
];

/** Built-in columns a viewer shows only after choosing them. */
export const defaultHiddenColumnIds: readonly string[] = [
  explorerColumnIds.systemTags,
];

// Extension integration literals.
export const explorerTableCellCapability = 'client.explorer_table_cell';
export const attricatRendererPrefix = 'attricat.';
export const extensionCellStartTimeout = 1_500;
// `flexRender` is only called for virtual rows. This bounded allocator keeps a
// pathological blueprint from turning one Explorer viewport into hundreds of
// opaque-origin frames.
export const maximumExplorerCellFrames = 32;

// Result table virtualization.
export const estimatedResultRowHeight = 53;
// Rows re-render only when their data changes, so a short overscan is enough
// to keep fast scrolling from showing blank space.
export const resultRowOverscan = 5;
/** Records previewed per blueprint when no blueprint is chosen. */
export const overviewRecordsPerBlueprint = 5;
/** Overview searches sent at once; the rest wait their turn. */
export const overviewConcurrentSearches = 4;
/** How long an overview stays fresh through focus changes and remounts. */
export const overviewStaleTimeMs = 30_000;

// Layout dimensions.
/** The record panel floats over the results; keep most of them visible. */
export const recordPanelWidth = 'clamp(420px, 36vw, 680px)';
/** Marks links that switch the panel to another record, so clicking one does not close it. */
export const recordPanelOpenerAttribute = 'data-record-panel-opener';
export const resultsTableHeight = {
  xs: 'calc(100dvh - 220px)',
  md: 'calc(100dvh - 165px)',
};
export const displayColumnMinWidth = 280;
export const browseRailWidth = 280;
/** One nesting level of the browse rail tree. */
export const browseRailIndent = 2;
/** Keeps leaf labels aligned with the labels of expandable nodes. */
export const browseRailToggleWidth = 28;
export const idColumnWidth = 48;
export const blueprintSelectWidth = 280;
export const searchScopePopoverWidth = 280;
export const systemTagPickerWidth = 320;
export const searchSyntaxPopoverMaxWidth = 440;
export const resultsLoadingIndicatorSize = 80;
export const columnPreferencesListMaxHeight = 480;
export const agentRecordListMaxHeight = 140;
export const selectedRecordListMaxHeight = 360;
export const selectedRecordListWidth = 360;
export const agentInstructionsRows = 3;
export const pendingVersionPlaceholder = '…';
export const loadMoreRowKey = 'load-more';
// Sticky header cells must stay above sticky body cells while scrolling.
export const stickyHeaderLayer = 3;
export const stickyBodyLayer = 1;

// Query autocomplete shows about eight suggestions before scrolling.
export const querySuggestionListMaxHeight = 360;
