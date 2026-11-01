import type { TFunction } from 'i18next';

/**
 * A permission's description in the user's language. The server describes
 * permissions in English only, so a permission the web app does not know yet
 * keeps that description.
 */
export const permissionDescription = (
  t: TFunction,
  permission: { code: string; description: string },
) =>
  t(
    `workspace.permissionDescriptions.${permission.code.replaceAll('.', '_')}`,
    { defaultValue: permission.description },
  );
