import { z } from 'zod';

/** A resource id for a request path; anything but a UUID is rejected before sending. */
export const uuidSegment = (id: string) =>
  encodeURIComponent(z.uuid().parse(id));

/** A revision number for a request path; it must be a positive integer. */
export const versionSegment = (version: number) =>
  z.number().int().positive().parse(version);

/** A revision number read from a route's `$version` path segment. */
export const versionParamSchema = z.coerce.number().int().positive();
