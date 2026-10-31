import { z } from 'zod';
import {
  cellDisplayTypes,
  contributionKinds,
  contributionKeyPattern,
  grantKinds,
  installationStates,
  layoutValidationMessages,
  maximumContributionKeyLength,
  maximumExtensionCommandIdLength,
  maximumExtensionStorageListLimit,
  minimumContributionKeyLength,
  navigationGroups,
  supportedWorkspaceLayoutVersion,
  extensionOutlets,
  hostIcons,
  maximumCellPrecision,
  maximumCellUnitLength,
  renderingModes,
} from './constants';

export const extensionOutletSchema = z.enum(extensionOutlets);

const contributionKeySchema = z
  .string()
  .min(minimumContributionKeyLength)
  .max(maximumContributionKeyLength)
  .regex(contributionKeyPattern);

/**
 * One dialog to open, one server command to call, or one interactive
 * operation to start, with the action context.
 */
const actionInvocationSchema = z.union([
  z.object({ dialog: z.string().min(1) }).strict(),
  z
    .object({ command: z.string().min(1).max(maximumExtensionCommandIdLength) })
    .strict(),
  z
    .object({
      operation: z
        .object({ id: z.string().min(1), input: z.unknown() })
        .strict(),
    })
    .strict(),
]);

/** When the host offers an action; every condition holds for every record. */
const actionConditionSchema = z
  .object({
    blueprints: z.array(z.string()).optional(),
    min_records: z.number().int().nonnegative().optional(),
    max_records: z.number().int().positive().optional(),
    requires_write: z.boolean().optional(),
    has_tags: z.array(z.string()).optional(),
    missing_tags: z.array(z.string()).optional(),
  })
  .strict();
export type ActionCondition = z.infer<typeof actionConditionSchema>;
export type ActionInvocation = z.infer<typeof actionInvocationSchema>;

/** A display setting: a literal, or the value of a prop the column passes. */
const cellSettingSchema = <T extends z.ZodType>(value: T) =>
  z.union([z.object({ prop: z.string().min(1) }).strict(), value]);

/** How the host draws a declared Explorer table cell. */
const cellSchema = z
  .object({
    value_types: z.array(z.string()).min(1),
    props: z.record(z.string(), z.unknown()).default({}),
    display: z.discriminatedUnion('type', [
      z
        .object({
          type: z.literal(cellDisplayTypes.number),
          precision: cellSettingSchema(
            z.number().int().min(0).max(maximumCellPrecision),
          ).optional(),
          unit: cellSettingSchema(
            z.string().min(1).max(maximumCellUnitLength),
          ).optional(),
        })
        .strict(),
    ]),
  })
  .strict();
export type CellDeclaration = z.infer<typeof cellSchema>;

/** The value types and props of a native attribute editor. */
const editorSchema = z
  .object({
    value_types: z.array(z.string()).min(1),
    props: z.record(z.string(), z.unknown()).default({}),
  })
  .strict();
export type EditorDeclaration = z.infer<typeof editorSchema>;

const contributionSchema = z
  .object({
    contribution_key: contributionKeySchema,
    display_order: z.number().int().nonnegative(),
    navigation_group: z.enum(navigationGroups).nullable(),
    extension_id: z.string().min(1),
    extension_name: z.string().min(1),
    release_id: z.uuid(),
    configuration: z.unknown(),
    capabilities: z.array(z.string()),
    id: z.string().min(1),
    version: z.number().int().positive(),
    kind: z.enum(contributionKinds),
    outlet: extensionOutletSchema.nullable(),
    route: z.string().nullable().default(null),
    title: z.string().nullable(),
    /** What a host-rendered action opens or calls; absent on other kinds. */
    invoke: actionInvocationSchema.nullable().default(null),
    when: actionConditionSchema.nullable().default(null),
    confirm: z
      .object({
        message: z.string().min(1),
        destructive: z.boolean().optional(),
      })
      .strict()
      .nullable()
      .default(null),
    icon: z.enum(hostIcons).nullable().default(null),
    /** The title in other languages, keyed by language code. */
    translations: z
      .record(z.string(), z.object({ title: z.string().min(1) }).strict())
      .default({}),
    /** How a host-rendered cell shows a value; absent on other kinds. */
    cell: cellSchema.nullable().default(null),
    /** An attribute editor's value types and props; absent on other kinds. */
    editor: editorSchema.nullable().default(null),
    /** Who draws the contribution's code; absent when the host draws it. */
    rendering: z.enum(renderingModes).nullable().default(null),
    /** The UI kit version native UI is drawn with. */
    ui_kit: z.string().nullable().default(null),
    /** Changes when a development build replaces the native Worker. */
    artifact_revision: z.string().nullable().default(null),
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
        message: layoutValidationMessages.duplicateKeys,
      });
    if (layout.hidden.some((key) => order.has(key)))
      context.addIssue({
        code: 'custom',
        message: layoutValidationMessages.hiddenAndOrdered,
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
          message: layoutValidationMessages.duplicateKeys,
        });
    if (layout.hidden.some((key) => layout.order.includes(key)))
      context.addIssue({
        code: 'custom',
        message: layoutValidationMessages.hiddenAndOrdered,
      });
    if (layout.hidden.some((key) => layout.promoted.includes(key)))
      context.addIssue({
        code: 'custom',
        message: layoutValidationMessages.hiddenAndPromoted,
      });
  });

export const workspaceExtensionLayoutSchema = z
  .object({
    version: z.literal(supportedWorkspaceLayoutVersion),
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
            outlet === extensionOutlets.navigation
              ? navigationLayoutSchema.safeParse(value)
              : baseOutletLayoutSchema.safeParse(value);
          if (!result.success) {
            context.addIssue({
              code: 'custom',
              message:
                result.error.issues[0]?.message ??
                layoutValidationMessages.invalidOutlet,
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
            message: layoutValidationMessages.multipleOutlets,
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
    command_id: z.string().min(1).max(maximumExtensionCommandIdLength),
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
      limit: z
        .number()
        .int()
        .min(1)
        .max(maximumExtensionStorageListLimit)
        .optional(),
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
  grant_kind: z.enum(grantKinds),
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
  actor_display_name: z.string().nullable(),
  actor_email: z.string().nullable(),
  actor_token_id: z.uuid().nullable(),
  source: z.string().nullable(),
  diagnostics: jsonValue,
  created_at: z.string(),
});
export const installationSchema = z.object({
  id: z.uuid(),
  extension_id: z.string(),
  installed_release_id: z.uuid(),
  state: z.enum(installationStates),
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
