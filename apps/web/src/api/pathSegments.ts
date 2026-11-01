import { z } from 'zod';
import { notFoundError } from './errors';

/**
 * A resource id for a request path. Anything but a UUID names no resource, so
 * it fails as not found before sending.
 */
export const uuidSegment = (id: string) => {
  const parsed = z.uuid().safeParse(id);
  if (!parsed.success) throw notFoundError();
  return encodeURIComponent(parsed.data);
};

/** A revision number for a request path; it must be a positive integer. */
export const versionSegment = (version: number) =>
  z.number().int().positive().parse(version);

/** A revision number read from a route's `$version` path segment. */
export const versionParamSchema = z.coerce.number().int().positive();
