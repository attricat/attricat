import { Button, Tooltip } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { RouterButton } from '../../components/RouterLink';
import { compactIconSize } from '../../components/iconSizes';
import { CommentIcon } from '../../components/systemIcons';
import { getCommentCount } from './api';
import { commentsSectionId } from './constants';
import { commentQueryKeys } from './queryKeys';
import { sessionOptions } from '../auth/queryOptions';

type Props = {
  recordId: string;
  /** Shows the comments where they are, instead of on the record page. */
  onOpen?: () => void;
};

/** Comment count leading to the record's comments; the label is in its tooltip. */
export const RecordCommentsLink = ({ recordId, onOpen }: Props) => {
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
  const icon = <CommentIcon size={compactIconSize} />;
  return (
    <Tooltip title={label}>
      {onOpen ? (
        <Button
          aria-label={label}
          onClick={onOpen}
          size="small"
          startIcon={icon}
          variant="text"
        >
          {count.data}
        </Button>
      ) : (
        <RouterButton
          aria-label={label}
          hash={commentsSectionId}
          params={{ recordId }}
          size="small"
          startIcon={icon}
          to="/records/$recordId"
          variant="text"
        >
          {count.data}
        </RouterButton>
      )}
    </Tooltip>
  );
};
