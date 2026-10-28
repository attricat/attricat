import { jsonRequest, request } from '../../api/request';
import {
  tagColorsPatchSchema,
  tagColorsSchema,
  type TagColorsPatch,
} from './schemas';

export type { TagColors, TagColorsPatch } from './schemas';

const tagColorsPath = '/api/workspace/tag-colors';

export const getTagColors = (signal?: AbortSignal) =>
  request(tagColorsPath, tagColorsSchema, { signal });

export const updateTagColors = (patch: TagColorsPatch) =>
  request(
    tagColorsPath,
    tagColorsSchema,
    jsonRequest('PATCH', tagColorsPatchSchema.parse(patch)),
  );
