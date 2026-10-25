import { z } from 'zod';

export const publicationChannelSchema = z.object({
  context_id: z.uuid(),
  context_code: z.string(),
  enabled: z.boolean(),
  /** Codes of enabled rules that must pass before publication. */
  required_rule_codes: z.array(z.string()).default([]),
  /** Whether the record schema and blueprint checks must pass. */
  require_valid_record: z.boolean().default(false),
});

/** Publication checks of a channel; omitted settings are kept by the API. */
export type PublicationChannelChecks = {
  required_rule_codes?: string[];
  require_valid_record?: boolean;
};

export type PublicationChannel = z.infer<typeof publicationChannelSchema>;
