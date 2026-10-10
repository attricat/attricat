export const tokenPermissionPresets = [
  {
    nameKey: 'profile.catalogGenerator',
    descriptionKey: 'profile.catalogGeneratorDescription',
    permissions: [
      'blueprints.read',
      'blueprints.write',
      'blueprints.publish',
      'contexts.read',
      'contexts.write',
      'records.read',
      'records.write',
      'records.publish',
    ],
  },
  {
    nameKey: 'profile.readOnlyCatalog',
    descriptionKey: 'profile.readOnlyCatalogDescription',
    permissions: ['blueprints.read', 'contexts.read', 'records.read'],
  },
  {
    nameKey: 'profile.recordImporter',
    descriptionKey: 'profile.recordImporterDescription',
    permissions: ['blueprints.read', 'contexts.read', 'records.write'],
  },
];

export const secretDialogMinWidth = 360;
export const profileTabIds = {
  tab: (index: number) => `profile-tab-${index}`,
  panel: (index: number) => `profile-tabpanel-${index}`,
};
export const profileTabIndex = {
  account: 0,
  personalTokens: 1,
  extensionRuns: 2,
} as const;
export const timeZonePickerMaxWidth = 480;
/** How often the time zone preview clock advances. */
export const previewRefreshMilliseconds = 30_000;
export const profileAvatarSize = 48;
/** Placeholder widths while the session loads. */
export const profileSkeletonWidth = 180;
export const profileValueSkeletonWidth = 240;
export const tokenSkeletonCount = 2;
export const tokenSkeletonHeight = 176;
export const tokensPath = '/profile/personal-access-tokens';
/** Forms keep a readable column inside the wider settings page. */
export const tokenFormMaxWidth = 720;
/** Mirrors the API's display name rules; the API remains authoritative. */
export const displayNameMinLength = 2;
export const displayNameMaxLength = 64;
/** Letters and digits in words separated by spaces. */
export const displayNameCharacters = /^[\p{Alphabetic}\p{N} ]*$/u;
/** Mirrors the API's avatar upload rules; the API remains authoritative. */
export const avatarMimeTypes = ['image/png', 'image/jpeg'];
export const avatarMaxBytes = 10 * 1024 * 1024;
export const avatarMaxMegabytes = avatarMaxBytes / (1024 * 1024);
/** How often the session refreshes while an uploaded avatar is processed. */
export const avatarPollMilliseconds = 1_000;
