export const PRINCIPAL_SCHEMA_KEY = 'x-attricat-principal';
export const DIRECTORY_PATH = '/api/directory';
export const TEAMS_PATH = '/api/workspace/teams';
/** Search filter value the server expands to the caller and their teams. */
export const CURRENT_USER_FILTER_VALUE = '@me';
export const principalKinds = { user: 'user', team: 'team' } as const;
/** The directory changes rarely; share one load across pickers and chips. */
export const DIRECTORY_STALE_TIME_MS = 60_000;
