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
      'entities.read',
      'entities.write',
      'entities.publish',
    ],
  },
  {
    nameKey: 'profile.readOnlyCatalog',
    descriptionKey: 'profile.readOnlyCatalogDescription',
    permissions: ['blueprints.read', 'contexts.read', 'entities.read'],
  },
  {
    nameKey: 'profile.entityImporter',
    descriptionKey: 'profile.entityImporterDescription',
    permissions: ['blueprints.read', 'contexts.read', 'entities.write'],
  },
];

export const secretDialogMinWidth = 360;
export const personalTokensHash = 'personal-api-tokens';
export const timeZonePickerMaxWidth = 480;
/** How often the time zone preview clock advances. */
export const previewRefreshMilliseconds = 30_000;
