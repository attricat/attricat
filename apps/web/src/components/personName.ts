/**
 * The name to show for a person: their display name, or `fallback` (such as
 * their email) when they have none or it is blank.
 */
export const personName = <T extends string | null | undefined>(
  displayName: string | null | undefined,
  fallback: T,
): string | T => displayName?.trim() || fallback;
