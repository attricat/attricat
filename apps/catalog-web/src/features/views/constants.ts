export const VIEW_COMPONENT_VERSION = 1;

export const VIEW_COMPONENT_IDS = {
  fieldDisplay: 'catalog.field_display',
  fieldEdit: 'catalog.field_edit',
  incomingRelationshipListDisplay: 'catalog.incoming_relationship_list_display',
  relationshipHierarchy: 'catalog.relationship_hierarchy',
  relationshipListDisplay: 'catalog.relationship_list_display',
  relationshipListEdit: 'catalog.relationship_list_edit',
  tableDisplay: 'catalog.table_display',
  tableEdit: 'catalog.table_edit',
  tableImage: 'catalog.table_image',
} as const;

export const HIERARCHY_PARENT_FIELD_PROP = 'parent_field';
export const ROOT_NODE_KEY = 'root';
export const VIEW_COMPONENT_LOG_LABEL = 'view component';
export const FILE_THUMBNAIL_SIZE = 64;
export const VIEW_SECTION_PADDING = 2.5;
export const VIEW_LAYOUT_SPACING = 2;
export const VIEW_GRID_COLUMNS = {
  xs: '1fr',
  md: 'repeat(2, minmax(0, 1fr))',
} as const;
export const HIERARCHY_FONT_SIZE = '0.875rem';
