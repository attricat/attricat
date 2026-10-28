import { z } from 'zod';
import { tagColorCodes, type TagColor } from '../../app/tagColors';

export const tagColorSchema = z.enum(tagColorCodes);

const isTagColor = (color: string): color is TagColor =>
  tagColorSchema.safeParse(color).success;

/** Tag colors by tag. A color this app does not know is left uncolored. */
const tagColorMapSchema = z
  .record(z.string(), z.string())
  .transform((colors) =>
    Object.fromEntries(
      Object.entries(colors).filter((entry): entry is [string, TagColor] =>
        isTagColor(entry[1]),
      ),
    ),
  );

export const tagColorsSchema = z.object({
  defaults: tagColorMapSchema,
  overrides: tagColorMapSchema,
});
export type TagColors = z.infer<typeof tagColorsSchema>;

export const tagColorsPatchSchema = z.object({
  set: z.record(z.string(), tagColorSchema).optional(),
  reset: z.array(z.string()).optional(),
});
export type TagColorsPatch = z.infer<typeof tagColorsPatchSchema>;
