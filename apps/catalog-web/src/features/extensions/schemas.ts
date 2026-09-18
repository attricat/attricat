import { z } from 'zod';

export const extensionOutletSchema = z.enum([
  'navigation',
  'entity_preview_panel',
  'blueprint_attribute_configuration',
  'entity_attribute_decoration',
  'entity_action',
  'explorer_row_action',
  'explorer_table_cell',
  'blueprint_detail_panel',
  'explorer_action',
  'explorer_bulk_action',
  'entity_header_action',
  'entity_attribute_panel',
  'blueprint_panel',
  'blueprint_publish_check',
  'file_panel',
  'audit_event_panel',
  'data_health_card',
]);

const contributionKeySchema = z
  .string()
  .min(3)
  .max(256)
  .regex(/^[A-Za-z0-9._-]+:[A-Za-z0-9._-]+$/);

const contributionSchema = z
  .object({
    contribution_key: contributionKeySchema,
    display_order: z.number().int().nonnegative(),
    navigation_group: z.enum(['promoted', 'grouped']).nullable(),
    extension_id: z.string().min(1),
    release_id: z.uuid(),
    configuration: z.unknown(),
    capabilities: z.array(z.string()),
    id: z.string().min(1),
    version: z.number().int().positive(),
    kind: z.enum(['route', 'navigation', 'embedded', 'action', 'panel']),
    outlet: extensionOutletSchema.nullable(),
    route: z.string().nullable().default(null),
    title: z.string().nullable(),
  })
  .strict();

export const runtimeSchema = z.array(contributionSchema);
export type ExtensionContribution = z.infer<typeof contributionSchema>;

const baseOutletLayoutSchema = z
  .object({
    order: z.array(contributionKeySchema),
    hidden: z.array(contributionKeySchema),
  })
  .strict()
  .superRefine((layout, context) => {
    const order = new Set(layout.order);
    const hidden = new Set(layout.hidden);
    if (
      order.size !== layout.order.length ||
      hidden.size !== layout.hidden.length
    )
      context.addIssue({
        code: 'custom',
        message: 'Contribution keys must be unique',
      });
    if (layout.hidden.some((key) => order.has(key)))
      context.addIssue({
        code: 'custom',
        message: 'A contribution cannot be ordered and hidden',
      });
  });

const navigationLayoutSchema = z
  .object({
    order: z.array(contributionKeySchema),
    hidden: z.array(contributionKeySchema),
    promoted: z.array(contributionKeySchema),
  })
  .strict()
  .superRefine((layout, context) => {
    for (const keys of [layout.order, layout.hidden, layout.promoted])
      if (new Set(keys).size !== keys.length)
        context.addIssue({
          code: 'custom',
          message: 'Contribution keys must be unique',
        });
    if (layout.hidden.some((key) => layout.order.includes(key)))
      context.addIssue({
        code: 'custom',
        message: 'A contribution cannot be ordered and hidden',
      });
    if (layout.hidden.some((key) => layout.promoted.includes(key)))
      context.addIssue({
        code: 'custom',
        message: 'A contribution cannot be hidden and promoted',
      });
  });

export const workspaceExtensionLayoutSchema = z
  .object({
    version: z.literal(1),
    outlets: z
      .partialRecord(extensionOutletSchema, z.unknown())
      .transform((outlets, context) => {
        const parsed: Record<
          string,
          | z.infer<typeof baseOutletLayoutSchema>
          | z.infer<typeof navigationLayoutSchema>
        > = {};
        for (const [outlet, value] of Object.entries(outlets)) {
          const result =
            outlet === 'navigation'
              ? navigationLayoutSchema.safeParse(value)
              : baseOutletLayoutSchema.safeParse(value);
          if (!result.success) {
            context.addIssue({
              code: 'custom',
              message:
                result.error.issues[0]?.message ?? 'Invalid outlet layout',
              path: [outlet],
            });
            return z.NEVER;
          }
          parsed[outlet] = result.data;
        }
        return parsed;
      }),
  })
  .strict()
  .superRefine((layout, context) => {
    const assigned = new Set<string>();
    for (const [outlet, item] of Object.entries(layout.outlets)) {
      for (const key of [...item.order, ...item.hidden]) {
        if (assigned.has(key)) {
          context.addIssue({
            code: 'custom',
            message: 'A contribution can be configured in only one outlet',
            path: ['outlets', outlet],
          });
        }
        assigned.add(key);
      }
    }
  });
export type WorkspaceExtensionLayout = z.infer<
  typeof workspaceExtensionLayoutSchema
>;

export const extensionCommandRequestSchema = z
  .object({
    release_id: z.uuid(),
    command_id: z.string().min(1).max(128),
    payload: z.unknown(),
  })
  .strict();

export const extensionStorageRequestSchema = z.discriminatedUnion('operation', [
  z.object({ operation: z.literal('get'), key: z.string() }).strict(),
  z
    .object({
      operation: z.literal('set'),
      key: z.string(),
      value: z.unknown(),
      expected_revision: z.number().int().positive().optional(),
    })
    .strict(),
  z
    .object({
      operation: z.literal('delete'),
      key: z.string(),
      expected_revision: z.number().int().positive().optional(),
    })
    .strict(),
  z
    .object({
      operation: z.literal('list'),
      prefix: z.string().optional(),
      cursor: z.string().optional(),
      limit: z.number().int().min(1).max(100).optional(),
    })
    .strict(),
]);
export type ExtensionStorageRequest = z.infer<
  typeof extensionStorageRequestSchema
>;

const jsonValue: z.ZodType<unknown> = z.lazy(() =>
  z.union([
    z.string(),
    z.number(),
    z.boolean(),
    z.null(),
    z.array(jsonValue),
    z.record(z.string(), jsonValue),
  ]),
);
const grantSchema = z.object({
  grant_kind: z.enum([
    'capability',
    'host_permission',
    'event_publish',
    'event_subscribe',
  ]),
  grant_id: z.string(),
  granted_at: z.string(),
});
const lifecycleSchema = z.object({
  id: z.uuid(),
  operation: z.string(),
  prior_state: z.string().nullable(),
  new_state: z.string().nullable(),
  outcome: z.string(),
  actor_user_id: z.uuid().nullable(),
  actor_token_id: z.uuid().nullable(),
  source: z.string().nullable(),
  diagnostics: jsonValue,
  created_at: z.string(),
});
export const installationSchema = z.object({
  id: z.uuid(),
  extension_id: z.string(),
  installed_release_id: z.uuid(),
  state: z.enum(['disabled', 'enabled', 'quarantined']),
  configuration: jsonValue,
  configuration_version: z.number().int().nullable(),
  created_at: z.string(),
  updated_at: z.string(),
  version: z.string(),
  manifest: jsonValue,
  manifest_sha256: z.string(),
  source: z.string(),
});
const detailSchema = z.object({
  installation: installationSchema,
  grants: z.array(grantSchema),
  lifecycle: z.array(lifecycleSchema),
});
export const discoveredSchema = z.object({
  registry_source: z.string(),
  id: z.string(),
  repository: z.string(),
  name: z.string(),
  description: z.string(),
  icon: z.string().nullable(),
});
const releaseSchema = z.object({
  source: z.string(),
  release_id: z.number().int(),
  tag_name: z.string(),
  name: z.string(),
  published_at: z.string().nullable(),
  asset: z.object({
    id: z.number().int(),
    name: z.string(),
    download_url: z.string(),
  }),
});
export const registryDetailsSchema = z.object({
  extension: discoveredSchema,
  readme: z.string(),
  releases: z.array(releaseSchema),
});
export { detailSchema };
export type ExtensionInstallation = z.infer<typeof installationSchema>;
export type ExtensionDetail = z.infer<typeof detailSchema>;
export type DiscoveredExtension = z.infer<typeof discoveredSchema>;
export type RegistryDetails = z.infer<typeof registryDetailsSchema>;
