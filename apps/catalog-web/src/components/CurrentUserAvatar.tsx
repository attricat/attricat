import { useQuery } from '@tanstack/react-query';
import type { ReactNode } from 'react';
import { currentSession } from '../features/auth/api';
import { authQueryKeys } from '../features/auth/queryKeys';
import { fileStatuses } from '../features/files/constants';
import { UserAvatar } from './UserAvatar';

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
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const account = session.data;
  if (account?.avatar?.status !== fileStatuses.ready) return fallback;
  return (
    <UserAvatar
      avatarFileId={account.avatar.file_id}
      name={account.display_name ?? account.email}
      size={size}
    />
  );
};
