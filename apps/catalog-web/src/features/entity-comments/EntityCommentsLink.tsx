import { Tooltip } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { RouterButton } from '../../components/RouterLink';
import { compactIconSize } from '../../components/iconSizes';
import { CommentIcon } from '../../components/systemIcons';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { getCommentCount } from './api';
import { commentsSectionId } from './constants';
import { commentQueryKeys } from './queryKeys';

/** Comment count linking to the comments on the entity page; the label is in its tooltip. */
export const EntityCommentsLink = ({ entityId }: { entityId: string }) => {
  const { t } = useTranslation();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const count = useQuery({
    queryKey: commentQueryKeys.count(
      session.data?.workspace_id,
      session.data?.user_id,
      entityId,
    ),
    queryFn: ({ signal }) => getCommentCount(entityId, signal),
    enabled: Boolean(session.data),
    select: (data) => data.count,
  });
  if (count.data === undefined) return null;
  const label = t('comments.count', { count: count.data });
  return (
    <Tooltip title={label}>
      <RouterButton
        aria-label={label}
        hash={commentsSectionId}
        params={{ entityId }}
        size="small"
        startIcon={<CommentIcon size={compactIconSize} />}
        to="/entities/$entityId"
        variant="text"
      >
        {count.data}
      </RouterButton>
    </Tooltip>
  );
};
