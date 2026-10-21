import { z } from 'zod';
import { request } from '../../api/request';
import { DIRECTORY_USERS_PATH } from './constants';
import { userProfileSchema } from './schemas';

export const getUserProfile = (userId: string, signal?: AbortSignal) =>
  request(
    `${DIRECTORY_USERS_PATH}/${z.uuid().parse(userId)}`,
    userProfileSchema,
    { signal },
  );
