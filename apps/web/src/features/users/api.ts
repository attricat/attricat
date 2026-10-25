import { request } from '../../api/request';
import { DIRECTORY_USERS_PATH } from './constants';
import { userProfileSchema } from './schemas';
import { uuidSegment } from '../../api/pathSegments';

export const getUserProfile = (userId: string, signal?: AbortSignal) =>
  request(`${DIRECTORY_USERS_PATH}/${uuidSegment(userId)}`, userProfileSchema, {
    signal,
  });
