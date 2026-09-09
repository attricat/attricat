import { z } from 'zod';

export const explorerTableCellContextSchema = z
  .object({
    context_version: z.literal(1),
    column: z
      .object({
        field: z.string().min(1),
        label: z.string().nullable(),
        renderer: z
          .object({
            id: z.string().min(1),
            version: z.number().int().positive(),
            props: z.record(z.string(), z.unknown()),
          })
          .strict(),
      })
      .strict(),
    primary_value: z.unknown(),
    related_entity: z
      .object({
        id: z.uuid(),
        blueprint_id: z.uuid(),
        blueprint_version: z.number().int().positive(),
        relationship_context_id: z.uuid(),
        relationship_context_code: z.string(),
      })
      .strict()
      .nullable(),
    related_preview: z.record(z.string(), z.unknown()).nullable(),
    source_row: z
      .object({
        entity_id: z.uuid(),
        blueprint_version: z.number().int().positive(),
        preview: z.record(z.string(), z.unknown()),
      })
      .strict(),
  })
  .strict();

export type ExplorerTableCellContext = z.infer<
  typeof explorerTableCellContextSchema
>;
