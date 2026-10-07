export const VIEW_COMPONENT_VERSION = 1;

export const VIEW_COMPONENT_IDS = {
  colorDisplay: 'catalog.color_display',
  colorEdit: 'catalog.color_edit',
  emailDisplay: 'catalog.email_display',
  emailEdit: 'catalog.email_edit',
  urlDisplay: 'catalog.url_display',
  urlEdit: 'catalog.url_edit',
  markdownDisplay: 'catalog.markdown_display',
  markdownEdit: 'catalog.markdown_edit',
  fieldDisplay: 'catalog.field_display',
  fieldEdit: 'catalog.field_edit',
  phoneDisplay: 'catalog.phone_display',
  phoneEdit: 'catalog.phone_edit',
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
export const TABLE_IMAGE_SIZE = 48;
export const VIEW_SECTION_PADDING = 2.5;
export const VIEW_LAYOUT_SPACING = 2;
/** Rows of editable fields need room for their floating labels and help text. */
export const VIEW_EDIT_LAYOUT_SPACING = 8;
export const VIEW_GRID_COLUMNS = {
  xs: '1fr',
  md: 'repeat(2, minmax(0, 1fr))',
} as const;
export const HIERARCHY_FONT_SIZE = '0.875rem';
