import { z } from 'zod';

export const userProfileSchema = z.object({
  id: z.uuid(),
  display_name: z.string().nullable(),
  email: z.string(),
  avatar_file_id: z.uuid().nullable(),
  active: z.boolean(),
  member_since: z.string(),
  teams: z.array(
    z.object({ id: z.uuid(), code: z.string(), name: z.string() }),
  ),
});
export type UserProfile = z.infer<typeof userProfileSchema>;
