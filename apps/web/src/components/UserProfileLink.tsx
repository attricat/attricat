import type { ReactNode } from 'react';
import { RouterLink } from './RouterLink';

/** A person's name as a link to their profile in the workspace. */
export const UserProfileLink = ({
  children,
  userId,
}: {
  children?: ReactNode;
  userId: string;
}) => (
  <RouterLink
    params={{ userId }}
    sx={{ color: 'inherit', fontWeight: 'inherit' }}
    to="/users/$userId"
    underline="hover"
  >
    {children}
  </RouterLink>
);
