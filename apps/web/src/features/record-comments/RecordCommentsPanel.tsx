import {
  useInfiniteQuery,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query';
import {
  Box,
  Button,
  CircularProgress,
  Paper,
  Stack,
  Typography,
} from '@mui/material';
import { useEffect, useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { QueryErrorNotice } from '../../components/QueryErrorNotice';
import { UserAvatar } from '../../components/UserAvatar';
import { Timestamp } from '../../time/Timestamp';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { listComments } from './api';
import { commentsSectionId } from './constants';
import { CommentComposer } from './CommentComposer';
import { MarkdownContent } from '../markdown/MarkdownContent';
import { commentQueryKeys } from './queryKeys';
import type { CommentCursor, RecordComment } from './schemas';
import { EmptyState } from '../../components/EmptyState';
import { CommentIcon } from '../../components/systemIcons';

const CommentItem = ({
  recordId,
  comment,
  userId,
  disabled,
  onSaved,
  onReload,
}: {
  recordId: string;
  comment: RecordComment;
  userId?: string;
  disabled: boolean;
  onSaved: () => void;
  onReload: () => Promise<unknown>;
}) => {
  const { t } = useTranslation();
  const [editing, setEditing] = useState<RecordComment | null>(null);
  const editButton = useRef<HTMLButtonElement>(null);
  const restoreFocus = useRef(false);
  // Closing can follow an async save, when the Edit button may not be
  // rendered by the next frame; focus it once the editor has unmounted.
  const close = () => {
    restoreFocus.current = true;
    setEditing(null);
  };
  useEffect(() => {
    if (editing || !restoreFocus.current) return;
    restoreFocus.current = false;
    editButton.current?.focus();
  }, [editing]);
  return (
    <Box component="li" sx={{ py: 2, borderBottom: 1, borderColor: 'divider' }}>
      <Stack
        direction="row"
        spacing={1}
        sx={{ alignItems: 'baseline', flexWrap: 'wrap', mb: 1 }}
      >
        <UserAvatar
          name={comment.author_display_name || comment.author_email}
          size={32}
        />
        <Typography variant="subtitle2">
          {comment.author_display_name || comment.author_email}
        </Typography>
        <Typography component="span" variant="caption" color="text.secondary">
          <Timestamp value={comment.created_at} />
        </Typography>
        {comment.revision > 1 && (
          <Typography component="span" variant="caption" color="text.secondary">
            {t('comments.edited')} <Timestamp value={comment.updated_at} />
          </Typography>
        )}
        {comment.author_user_id === userId && !editing && (
          <Button
            ref={editButton}
            disabled={disabled}
            onClick={() => setEditing(comment)}
          >
            {t('comments.edit')}
          </Button>
        )}
      </Stack>
      {editing ? (
        <CommentComposer
          recordId={recordId}
          comment={editing}
          disabled={disabled}
          onSaved={() => {
            close();
            onSaved();
          }}
          onCancel={close}
          onReload={async () => {
            await onReload();
            close();
          }}
        />
      ) : (
        <MarkdownContent value={comment.body} />
      )}
    </Box>
  );
};

export const RecordCommentsPanel = ({ recordId }: { recordId: string }) => {
  const { t } = useTranslation();
  const headingId = useId();
  const client = useQueryClient();
  const [composerVersion, setComposerVersion] = useState(0);
  const [notice, setNotice] = useState('');
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const key = commentQueryKeys.record(
    session.data?.workspace_id,
    session.data?.user_id,
    recordId,
  );
  const comments = useInfiniteQuery({
    queryKey: key,
    queryFn: ({ pageParam }) => listComments(recordId, pageParam),
    initialPageParam: undefined as CommentCursor | undefined,
    getNextPageParam: (page) => {
      const last = page.items.at(-1);
      return page.has_more && last
        ? { before_time: last.created_at, before_id: last.id }
        : undefined;
    },
    enabled: Boolean(session.data),
  });
  const saved = () => {
    setNotice(t('comments.saved'));
    void client.invalidateQueries({ queryKey: key });
  };
  const items = comments.data?.pages.flatMap((page) => page.items) ?? [];
  const sectionRef = useRef<HTMLElement>(null);
  // The section loads after the record, too late for the router's own
  // scroll to a link's #comments target.
  useEffect(() => {
    if (window.location.hash === `#${commentsSectionId}`)
      sectionRef.current?.scrollIntoView();
  }, []);
  return (
    <Paper
      component="section"
      aria-labelledby={headingId}
      id={commentsSectionId}
      ref={sectionRef}
      sx={{ mt: 3, p: { xs: 2, md: 3 } }}
    >
      <Typography id={headingId} component="h2" variant="h6" sx={{ mb: 2 }}>
        {t('comments.title')}
      </Typography>
      <Typography role="status" variant="body2">
        {notice}
      </Typography>
      {(comments.isPending || session.isPending) && (
        <CircularProgress aria-label={t('comments.loading')} />
      )}
      <QueryErrorNotice
        error={comments.error ?? session.error}
        isRetrying={comments.isFetching || session.isFetching}
        onRetry={() => {
          void session.refetch();
          void comments.refetch();
        }}
      />
      {comments.isSuccess && items.length === 0 && (
        <EmptyState icon={CommentIcon} title={t('comments.empty')} />
      )}
      {session.data && (
        <CommentComposer
          key={`${session.data.workspace_id}:${session.data.user_id}:${recordId}:${composerVersion}`}
          recordId={recordId}
          disabled={!comments.data || comments.isError}
          onSaved={() => {
            setComposerVersion((version) => version + 1);
            saved();
          }}
        />
      )}
      <Box component="ul" sx={{ listStyle: 'none', p: 0, m: 0 }}>
        {items.map((comment) => (
          <CommentItem
            key={comment.id}
            recordId={recordId}
            comment={comment}
            userId={session.data?.user_id}
            disabled={comments.isError}
            onSaved={saved}
            onReload={() => comments.refetch()}
          />
        ))}
      </Box>
      {comments.hasNextPage && (
        <Button
          disabled={comments.isFetchingNextPage}
          onClick={() => void comments.fetchNextPage()}
        >
          {t('comments.loadMore')}
        </Button>
      )}
    </Paper>
  );
};
