import { useQuery } from '@tanstack/react-query';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';

/** The workspace and user whose inbox the current session shows. */
export const useInboxIdentity = () => {
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  return {
    session,
    workspaceId: session.data?.workspace_id,
    userId: session.data?.user_id,
  };
};
