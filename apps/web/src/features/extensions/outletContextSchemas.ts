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
