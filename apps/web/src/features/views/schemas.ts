import { z } from 'zod';
import { MAX_COMPONENT_PROTOCOL_VERSION } from '../records/constants';

export const viewBlockTypes = {
  dropdownOption: 'dropdown_option',
  pageTitle: 'page_title',
  stack: 'stack',
  grid: 'grid',
  section: 'section',
  tabs: 'tabs',
  accordion: 'accordion',
  heading: 'heading',
  text: 'text',
  divider: 'divider',
  field: 'field',
  relationshipList: 'relationship_list',
  incomingRelationshipList: 'incoming_relationship_list',
  table: 'table',
  extensionLayout: 'extension_layout',
  explorer: 'explorer',
} as const;

export type ComponentReference = {
  id: string;
  version: number;
  props: Record<string, unknown>;
};
/** An extension's attribute editor a detail field names. */
export type EditorReference = ComponentReference;
export type ViewNode =
  | {
      type: 'stack' | 'grid' | 'section';
      children: ViewNode[];
      component?: ComponentReference | null;
    }
  | {
      type: 'tabs';
      tabs: { label: string; children: ViewNode[] }[];
      component?: ComponentReference | null;
    }
  | {
      type: 'accordion';
      sections: { label: string; children: ViewNode[] }[];
      component?: ComponentReference | null;
    }
  | {
      type: 'heading' | 'text';
      text: string;
      component?: ComponentReference | null;
    }
  | { type: 'divider'; component?: ComponentReference | null }
  | {
      type: 'field';
      field: string;
      component?: ComponentReference | null;
      editor?: EditorReference | null;
    }
  | {
      type: 'relationship_list';
      field: string;
      component?: ComponentReference | null;
    }
  | {
      type: 'incoming_relationship_list';
      label: string;
      relationships: { source_blueprint: string; field: string }[];
      page_size: number;
      component?: ComponentReference | null;
    };
export type ViewDefinition =
  | {
      type: 'dropdown_option';
      fields: string[];
      separator?: string;
    }
  | {
      type: 'page_title';
      fields: string[];
      separator?: string;
    }
  | {
      type: 'table';
      columns?: {
        field: string;
        label?: string | null;
        renderer?: ComponentReference | null;
      }[];
      component?: ComponentReference | null;
    }
  | { type: 'explorer'; browse_by?: string | false | null }
  | {
      type: 'extension_layout';
      version: 1;
      outlets: Record<string, { order: string[]; hidden: string[] }>;
    }
  | Exclude<
      ViewNode,
      {
        type:
          | 'heading'
          | 'text'
          | 'divider'
          | 'field'
          | 'relationship_list'
          | 'incoming_relationship_list';
      }
    >;

export const jsonObjectSchema = z.record(z.string(), z.unknown());
const componentReferenceSchema = z.object({
  id: z.string().regex(/^[a-z][a-z0-9_-]*(\.[a-z][a-z0-9_-]*)*$/),
  // Rust stores component protocol versions as u32; reject values that would
  // otherwise overflow the API contract.
  version: z.number().int().positive().max(MAX_COMPONENT_PROTOCOL_VERSION),
  props: jsonObjectSchema.nullish().transform((props) => props ?? {}),
});
// Editor IDs are extension contribution IDs.
const editorReferenceSchema = componentReferenceSchema.extend({
  id: z.string().regex(/^[A-Za-z0-9._-]{1,128}$/),
});
/**
 * Blocks that lay out child nodes; they may appear nested in a view or as a
 * view's root. Built on demand because they refer back to `viewNodeSchema`.
 */
const containerBlockSchemas = () =>
  [
    z.object({
      type: z.literal(viewBlockTypes.stack),
      children: z.array(viewNodeSchema),
      component: componentReferenceSchema.nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.grid),
      children: z.array(viewNodeSchema),
      component: componentReferenceSchema.nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.section),
      children: z.array(viewNodeSchema),
      component: componentReferenceSchema.nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.tabs),
      tabs: z.array(
        z.object({ label: z.string(), children: z.array(viewNodeSchema) }),
      ),
      component: componentReferenceSchema.nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.accordion),
      sections: z.array(
        z.object({ label: z.string(), children: z.array(viewNodeSchema) }),
      ),
      component: componentReferenceSchema.nullish(),
    }),
  ] as const;
const viewNodeSchema: z.ZodType<ViewNode> = z.lazy(() =>
  z.discriminatedUnion('type', [
    ...containerBlockSchemas(),
    z.object({
      type: z.literal(viewBlockTypes.heading),
      text: z.string(),
      component: componentReferenceSchema.nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.text),
      text: z.string(),
      component: componentReferenceSchema.nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.divider),
      component: componentReferenceSchema.nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.field),
      field: z.string(),
      component: componentReferenceSchema.nullish(),
      editor: editorReferenceSchema.nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.relationshipList),
      field: z.string(),
      component: componentReferenceSchema.nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.incomingRelationshipList),
      label: z.string(),
      relationships: z.array(
        z.object({ source_blueprint: z.string(), field: z.string() }),
      ),
      page_size: z.number().int().positive(),
      component: componentReferenceSchema.nullish(),
    }),
  ]),
);
/** A `dropdown_option` or `page_title` view: field values joined into one label. */
export const fieldListViewSchema = z.object({
  fields: z.array(z.string()),
  separator: z.string().optional(),
});
const viewDefinitionSchema: z.ZodType<ViewDefinition> = z.lazy(() =>
  z.union([
    fieldListViewSchema.extend({
      type: z.literal(viewBlockTypes.dropdownOption),
    }),
    fieldListViewSchema.extend({
      type: z.literal(viewBlockTypes.pageTitle),
    }),
    z.object({
      type: z.literal(viewBlockTypes.table),
      columns: z
        .array(
          z.object({
            field: z.string(),
            label: z.string().nullable().optional(),
            renderer: componentReferenceSchema.nullish(),
          }),
        )
        .nullish()
        .transform((columns) => columns ?? undefined),
      component: componentReferenceSchema.nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.explorer),
      browse_by: z.union([z.string(), z.literal(false)]).nullish(),
    }),
    z.object({
      type: z.literal(viewBlockTypes.extensionLayout),
      version: z.literal(1),
      outlets: z.record(
        z.string(),
        z.object({ order: z.array(z.string()), hidden: z.array(z.string()) }),
      ),
    }),
    ...containerBlockSchemas(),
  ]),
);
export const viewsSchema = z.record(z.string(), viewDefinitionSchema);
