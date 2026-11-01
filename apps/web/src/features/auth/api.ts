import {
  ApiRequestError,
  request,
  requestNoContent,
  requestUpload,
  jsonRequest,
} from '../../api/request';
import { httpStatus } from '../../api/httpStatus';
import type { AccessTargetKind } from './capabilities';
import {
  ownAvatarSchema,
  sessionSchema,
  discoverySchema,
  sampleLoginsSchema,
  singleSignOnStartSchema,
  invitationOptionsSchema,
  identityLinkStatusSchema,
  type UserPreferences,
} from './schemas';
import { accessSchema } from './schemas';
export type { Session, SampleLogins } from './schemas';

export const fetchSampleLogins = async () =>
  request('/api/auth/sample-logins', sampleLoginsSchema);

export const discoverWorkspace = async (loginIdentifier: string) => {
  return request(
    '/api/auth/discover',
    discoverySchema,
    jsonRequest('POST', { login_identifier: loginIdentifier }),
  );
};

export const login = async (
  loginIdentifier: string,
  email: string,
  password: string,
) => {
  return request(
    '/api/auth/login',
    sessionSchema,
    jsonRequest('POST', {
      login_identifier: loginIdentifier,
      email,
      password,
    }),
  );
};

export const startSingleSignOn = (
  loginIdentifier: string,
  invitationSecret?: string,
) =>
  request(
    '/api/auth/oidc/start',
    singleSignOnStartSchema,
    jsonRequest('POST', {
      login_identifier: loginIdentifier,
      ...(invitationSecret ? { invitation_secret: invitationSecret } : {}),
    }),
  );

/** How an invitation can be accepted, or `null` for an unusable secret. */
export const getInvitationOptions = async (secret: string) => {
  try {
    return await request(
      '/api/auth/oidc/invitation',
      invitationOptionsSchema,
      jsonRequest('POST', { secret }),
    );
  } catch (error) {
    if (
      error instanceof ApiRequestError &&
      error.status === httpStatus.unprocessableEntity
    )
      return null;
    throw error;
  }
};

/** Starts linking the signed-in account to the workspace's provider. */
export const startIdentityLink = () =>
  request(
    '/api/auth/oidc/link',
    singleSignOnStartSchema,
    jsonRequest('POST', {}),
  );

/** The caller's link to the workspace's provider, or `null` without one. */
export const getIdentityLinkStatus = async () => {
  try {
    return await request('/api/auth/oidc/link', identityLinkStatusSchema);
  } catch (error) {
    if (
      error instanceof ApiRequestError &&
      error.status === httpStatus.notFound
    )
      return null;
    throw error;
  }
};

export const requestPasswordReset = async (email: string) => {
  return requestNoContent(
    '/api/auth/password-reset',
    jsonRequest('POST', { email }),
  );
};

export const confirmPasswordReset = async (token: string, password: string) => {
  return requestNoContent(
    '/api/auth/password-reset/confirm',
    jsonRequest('POST', { token, password }),
  );
};

/** The most targets one access check may name. */
export const maxAccessTargets = 500;

/** Which of `ids` the caller holds `permission` on. */
export const checkAccess = (
  permission: string,
  kind: AccessTargetKind,
  ids: readonly string[],
) =>
  request(
    '/api/auth/access',
    accessSchema,
    jsonRequest('POST', { permission, kind, ids }),
  );

export const currentSession = async () => {
  try {
    return await request('/api/auth/session', sessionSchema);
  } catch (error) {
    if (
      error instanceof ApiRequestError &&
      error.status === httpStatus.unauthorized
    )
      return null;
    throw error;
  }
};

export const updatePreferences = (preferences: UserPreferences) =>
  request(
    '/api/auth/preferences',
    sessionSchema,
    jsonRequest('PATCH', preferences),
  );

export const updateDisplayName = (displayName: string) =>
  request(
    '/api/auth/display-name',
    sessionSchema,
    jsonRequest('PUT', { display_name: displayName }),
  );

export const clearDisplayName = () =>
  request('/api/auth/display-name', sessionSchema, { method: 'DELETE' });

export const uploadAvatar = (
  file: File,
  onProgress?: (progress: number) => void,
) => {
  const data = new FormData();
  data.append('file', file);
  return requestUpload(
    '/api/auth/avatar',
    data,
    ownAvatarSchema,
    onProgress,
    'PUT',
  );
};

export const removeAvatar = () =>
  requestNoContent('/api/auth/avatar', { method: 'DELETE' });

export const logout = () =>
  requestNoContent('/api/auth/logout', { method: 'POST' });
