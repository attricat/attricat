import { personName } from './personName';
import { useQuery } from '@tanstack/react-query';
import type { ReactNode } from 'react';
import { fileStatuses } from '../features/files/constants';
import { UserAvatar } from './UserAvatar';
import { sessionOptions } from '../features/auth/queryOptions';

/**
 * The signed-in user's avatar photo, or `fallback` until they have a
 * processed one. Surfaces keep their own placeholder rather than initials.
 */
export const CurrentUserAvatar = ({
  fallback,
  size,
}: {
  fallback: ReactNode;
  size: number;
}) => {
  const session = useQuery(sessionOptions());
  const account = session.data;
  if (account?.avatar?.status !== fileStatuses.ready) return fallback;
  return (
    <UserAvatar
      avatarFileId={account.avatar.file_id}
      name={personName(account.display_name, account.email)}
      size={size}
    />
  );
};
