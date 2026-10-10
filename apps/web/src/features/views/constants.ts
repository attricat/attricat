export const VIEW_COMPONENT_VERSION = 1;

export const VIEW_COMPONENT_IDS = {
  colorDisplay: 'attricat.color_display',
  colorEdit: 'attricat.color_edit',
  emailDisplay: 'attricat.email_display',
  emailEdit: 'attricat.email_edit',
  urlDisplay: 'attricat.url_display',
  urlEdit: 'attricat.url_edit',
  markdownDisplay: 'attricat.markdown_display',
  markdownEdit: 'attricat.markdown_edit',
  fieldDisplay: 'attricat.field_display',
  fieldEdit: 'attricat.field_edit',
  phoneDisplay: 'attricat.phone_display',
  phoneEdit: 'attricat.phone_edit',
  incomingRelationshipListDisplay: 'attricat.incoming_relationship_list_display',
  relationshipHierarchy: 'attricat.relationship_hierarchy',
  relationshipListDisplay: 'attricat.relationship_list_display',
  relationshipListEdit: 'attricat.relationship_list_edit',
  tableDisplay: 'attricat.table_display',
  tableEdit: 'attricat.table_edit',
  tableImage: 'attricat.table_image',
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
