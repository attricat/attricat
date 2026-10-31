import { z } from 'zod';
import { supportedOutletContextVersion } from './constants';

const contextVersion = z.literal(supportedOutletContextVersion);

/** A selection of one to `maximum` distinct records. */
export const uniqueRecordIdsSchema = (maximum: number) =>
  z
    .array(z.uuid())
    .min(1)
    .max(maximum)
    .refine((ids) => new Set(ids).size === ids.length);
const blueprintVersion = z.number().int().positive();
/** An extension's export id, as its manifest declares it. */
const exportIdPattern = /^[A-Za-z0-9._-]{1,128}$/;

const blueprintRevisionContextSchema = z
  .object({
    blueprint_id: z.uuid(),
    blueprint_version: blueprintVersion,
    context_version: contextVersion,
  })
  .strict();

// New outlet contexts are deliberately small, strict, and versioned. They are
// the only page data an extension frame receives for these surfaces.
export const outletContextSchemas = {
  blueprint_publish_check: blueprintRevisionContextSchema,
  data_health_card: z.object({ context_version: contextVersion }).strict(),
  export_mapping: z
    .object({
      context_version: contextVersion,
      export_id: z.string().regex(exportIdPattern),
    })
    .strict(),
  audit_event_panel: z
    .object({
      context_version: contextVersion,
      event_id: z.uuid(),
    })
    .strict(),
  blueprint_panel: blueprintRevisionContextSchema,
  blueprint_detail_panel: blueprintRevisionContextSchema,
};

/**
 * What a native attribute editor sees: the value, the attribute's definition,
 * the field's state and the blueprint's props for the editor. The host owns
 * the label, errors, focus and saving; the editor only proposes values.
 */
export const attributeEditorContextSchema = z
  .object({
    context_version: contextVersion,
    record_id: z.uuid(),
    context_id: z.uuid().nullable(),
    /** The field's value as the inline editor holds it, or null when empty. */
    value: z.union([z.string(), z.number(), z.boolean()]).nullable(),
    attribute: z
      .object({
        code: z.string(),
        name: z.string(),
        value_type: z.string(),
        /** The attribute's JSON Schema constraints, when it has any. */
        constraints: z.record(z.string(), z.unknown()).nullable(),
        /** Allowed values, for an attribute limited to a list. */
        options: z.array(z.union([z.string(), z.number()])).nullable(),
      })
      .strict(),
    read_only: z.boolean(),
    required: z.boolean(),
    errors: z.array(z.string()),
    props: z.record(z.string(), z.unknown()),
  })
  .strict();
export type AttributeEditorContext = z.infer<
  typeof attributeEditorContextSchema
>;
