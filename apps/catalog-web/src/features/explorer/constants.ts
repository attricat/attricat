// Search and filter limits.
export const maximumAttributeFilters = 20;
export const maximumAgentSelection = 50;
export const maximumAgentInstructionsLength = 1_000;
export const maximumAgentConversationBlueprintNameLength = 120;
export const maximumAgentEntityLabelLength = 60;

// Storage keys.
export const lastBlueprintStorageKey = 'catalog.explorer.last-blueprint';
export const columnPreferencesStorageKeyPrefix =
  'catalog.explorer.column-preferences.';

// Attribute visibility scope used to hide attributes from the Explorer.
export const explorerVisibilityScope = 'explorer';

// Relationship paths such as `brand.country` join attribute codes with a dot.
export const relationshipPathSeparator = '.';

export const defaultAttributeFilterOperator = 'eq';
/** Cell shortcuts always filter by equality rather than guessing an operator. */
export const cellFilterOperator = 'eq';

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
  publicationStatus: 'publication_status',
} as const;

export const publicationStatuses = {
  notPublished: 'not_published',
  published: 'published',
} as const;

export const publishedRevisionStatus = 'published';

export const relationshipPathSortErrorCode =
  'relationship_path_sort_requires_single_result_version';

export const explorerColumnIds = {
  actions: 'actions',
  display: 'display',
  id: 'id',
  publication: 'publication',
  schema: 'schema',
  select: 'select',
} as const;

export const builtInConfigurableColumnIds: string[] = [
  explorerColumnIds.id,
  explorerColumnIds.display,
  explorerColumnIds.publication,
  explorerColumnIds.schema,
];

// Extension integration literals.
export const explorerExtensionContextVersion = 1;
export const explorerExtensionOutlets = {
  action: 'explorer_action',
  bulkAction: 'explorer_bulk_action',
  rowAction: 'explorer_row_action',
  tableCell: 'explorer_table_cell',
} as const;
export const explorerTableCellCapability = 'client.explorer_table_cell';
export const embeddedExtensionKind = 'embedded';
export const catalogRendererPrefix = 'catalog.';
export const extensionCellStartTimeout = 1_500;
// `flexRender` is only called for virtual rows. This bounded allocator keeps a
// pathological blueprint from turning one Explorer viewport into hundreds of
// opaque-origin frames.
export const maximumExplorerCellFrames = 32;

// Result table virtualization.
export const estimatedResultRowHeight = 53;
export const resultRowOverscan = 10;

// Layout dimensions.
export const facetSidebarWidth = 300;
export const facetSidebarStickyTop = 88;
export const facetSidebarMaxHeight = 'calc(100dvh - 104px)';
export const facetHeadingIconSize = 15;
export const resultsTableHeight = {
  xs: 'calc(100dvh - 220px)',
  md: 'calc(100dvh - 165px)',
};
export const displayColumnMinWidth = 280;
export const idColumnWidth = 48;
export const blueprintSelectWidth = 280;
export const versionScopeSelectMinWidth = 190;
export const searchSyntaxPopoverMaxWidth = 440;
export const resultsLoadingIndicatorSize = 80;
export const columnPreferencesListMaxHeight = 480;
export const agentEntityListMaxHeight = 140;
export const selectedEntityListMaxHeight = 360;
export const selectedEntityListWidth = 360;
export const agentInstructionsRows = 3;
export const pendingVersionPlaceholder = '…';
export const emptyValuePlaceholder = '—';
export const loadMoreRowKey = 'load-more';
// Sticky header cells must stay above sticky body cells while scrolling.
export const stickyHeaderLayer = 3;
export const stickyBodyLayer = 1;

// Query autocomplete shows about eight suggestions before scrolling.
export const querySuggestionListMaxHeight = 360;
