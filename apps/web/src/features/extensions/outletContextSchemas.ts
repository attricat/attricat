import { z } from 'zod';
import { maximumAgentSelection } from '../explorer/agentSelection';
import { supportedOutletContextVersion } from './constants';

const contextVersion = z.literal(supportedOutletContextVersion);
const blueprintVersion = z.number().int().positive();

const blueprintRevisionContextSchema = z
  .object({
    blueprint_id: z.uuid(),
    blueprint_version: blueprintVersion,
    context_version: contextVersion,
  })
  .strict();

const recordRevisionContextSchema = z
  .object({
    blueprint_id: z.uuid(),
    blueprint_version: blueprintVersion,
    context_version: contextVersion,
    record_id: z.uuid(),
  })
  .strict();

// New outlet contexts are deliberately small, strict, and versioned. They are
// the only page data an extension frame receives for these surfaces.
export const outletContextSchemas = {
  file_panel: z
    .object({
      context_version: contextVersion,
      file_id: z.uuid(),
      record_id: z.uuid(),
      attribute_id: z.uuid(),
      blueprint_id: z.uuid(),
      blueprint_version: blueprintVersion,
    })
    .strict(),
  record_attribute_panel: z
    .object({
      context_version: contextVersion,
      record_id: z.uuid(),
      attribute_id: z.uuid(),
      blueprint_id: z.uuid(),
      blueprint_version: blueprintVersion,
      context_id: z.uuid().nullable(),
    })
    .strict(),
  blueprint_publish_check: blueprintRevisionContextSchema,
  data_health_card: z.object({ context_version: contextVersion }).strict(),
  explorer_bulk_action: z
    .object({
      context_version: contextVersion,
      blueprint_id: z.uuid(),
      blueprint_version: blueprintVersion,
      record_ids: z
        .array(z.uuid())
        .min(1)
        .max(maximumAgentSelection)
        .refine((ids) => new Set(ids).size === ids.length),
    })
    .strict(),
  explorer_action: blueprintRevisionContextSchema,
  audit_event_panel: z
    .object({
      context_version: contextVersion,
      event_id: z.uuid(),
    })
    .strict(),
  record_header_action: recordRevisionContextSchema,
  explorer_row_action: recordRevisionContextSchema,
  blueprint_panel: blueprintRevisionContextSchema,
  blueprint_detail_panel: blueprintRevisionContextSchema,
};
