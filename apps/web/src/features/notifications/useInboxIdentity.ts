import { useQuery } from '@tanstack/react-query';
import { sessionOptions } from '../auth/queryOptions';

/** The workspace and user whose inbox the current session shows. */
export const useInboxIdentity = () => {
  const session = useQuery(sessionOptions());
  return {
    session,
    workspaceId: session.data?.workspace_id,
    userId: session.data?.user_id,
  };
};
