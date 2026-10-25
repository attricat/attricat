import { Tooltip } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { RouterButton } from '../../components/RouterLink';
import { compactIconSize } from '../../components/iconSizes';
import { CommentIcon } from '../../components/systemIcons';
import { getCommentCount } from './api';
import { commentsSectionId } from './constants';
import { commentQueryKeys } from './queryKeys';
import { sessionOptions } from '../auth/queryOptions';

/** Comment count linking to the comments on the record page; the label is in its tooltip. */
export const RecordCommentsLink = ({ recordId }: { recordId: string }) => {
  const { t } = useTranslation();
  const session = useQuery(sessionOptions());
  const count = useQuery({
    queryKey: commentQueryKeys.count(
      session.data?.workspace_id,
      session.data?.user_id,
      recordId,
    ),
    queryFn: ({ signal }) => getCommentCount(recordId, signal),
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
        params={{ recordId }}
        size="small"
        startIcon={<CommentIcon size={compactIconSize} />}
        to="/records/$recordId"
        variant="text"
      >
        {count.data}
      </RouterButton>
    </Tooltip>
  );
};
