import { z } from 'zod';
import { principalKinds } from './constants';

const uuid = z.uuid();

export const principalConfigurationSchema = z
  .object({
    version: z.literal(1),
    kinds: z
      .array(z.enum([principalKinds.user, principalKinds.team]))
      .min(1)
      .max(2),
  })
  .strict();
export type PrincipalConfiguration = z.infer<
  typeof principalConfigurationSchema
>;
export type PrincipalKind = PrincipalConfiguration['kinds'][number];

export const directoryUserSchema = z.object({
  id: uuid,
  display_name: z.string().nullable(),
  email: z.string(),
  active: z.boolean(),
});
export const directoryTeamSchema = z.object({
  id: uuid,
  code: z.string(),
  name: z.string(),
  deleted: z.boolean(),
});
export const directorySchema = z.object({
  users: z.array(directoryUserSchema),
  teams: z.array(directoryTeamSchema),
});
export type Directory = z.infer<typeof directorySchema>;
export type DirectoryUser = z.infer<typeof directoryUserSchema>;
export type DirectoryTeam = z.infer<typeof directoryTeamSchema>;

export const teamSchema = z.object({
  id: uuid,
  code: z.string(),
  name: z.string(),
  member_user_ids: z.array(uuid),
  created_at: z.string(),
  updated_at: z.string(),
});
export type Team = z.infer<typeof teamSchema>;
