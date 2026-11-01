import { personName } from '../../components/personName';
import { NextPageButton } from '../../components/LoadMoreButton';
import {
  useInfiniteQuery,
  useMutation,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query';
import {
  Box,
  Button,
  Chip,
  CircularProgress,
  Stack,
  Typography,
} from '@mui/material';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { QueryErrorNotice } from '../../components/QueryErrorNotice';
import { BrandIcon } from '../../components/BrandIcon';
import { UserAvatar } from '../../components/UserAvatar';
import { UserProfileLink } from '../../components/UserProfileLink';
import { Timestamp } from '../../time/Timestamp';
import { commentsSectionId, systemAuthorKind } from './constants';
import { CommentComposer } from './CommentComposer';
import { MarkdownContent } from '../markdown/MarkdownContent';
import { recordCommentsOptions } from './queryOptions';
import type { RecordComment } from './schemas';
import { EmptyState } from '../../components/EmptyState';
import { CommentIcon } from '../../components/systemIcons';
import { sessionOptions } from '../auth/queryOptions';
import { userAvatarSizes } from '../../components/userAvatars';
import { PencilIcon, Trash2Icon } from 'lucide-react';
import { ConfirmDialog } from '../../components/ConfirmDialog';
import { deleteComment } from './api';

const CommentItem = ({
  recordId,
  comment,
  userId,
  disabled,
  onSaved,
  onReload,
  onDeleted,
}: {
  recordId: string;
  comment: RecordComment;
  userId?: string;
  disabled: boolean;
  onSaved: () => void;
  onReload: () => Promise<unknown>;
  onDeleted: () => void;
}) => {
  const { t } = useTranslation();
  const [editing, setEditing] = useState<RecordComment | null>(null);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const remove = useMutation({
    mutationFn: () => deleteComment(recordId, comment.id, comment.revision),
    onSuccess: () => {
      setConfirmingDelete(false);
      onDeleted();
    },
  });
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
        {comment.author_kind === systemAuthorKind ? (
          <BrandIcon />
        ) : (
          <UserAvatar
            name={personName(comment.author_display_name, comment.author_email)}
            size={userAvatarSizes.message}
          />
        )}
        <Typography variant="subtitle2">
          {comment.author_kind === systemAuthorKind ? (
            personName(comment.author_display_name, comment.author_email)
          ) : (
            <UserProfileLink userId={comment.author_user_id}>
              {personName(comment.author_display_name, comment.author_email)}
            </UserProfileLink>
          )}
        </Typography>
        {comment.author_kind === systemAuthorKind && (
          <Chip
            label={t('comments.systemAuthor')}
            size="small"
            variant="outlined"
          />
        )}
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
            startIcon={<PencilIcon />}
          >
            {t('comments.edit')}
          </Button>
        )}
        {comment.author_user_id === userId && !editing && (
          <Button
            color="error"
            disabled={disabled}
            onClick={() => {
              remove.reset();
              setConfirmingDelete(true);
            }}
            startIcon={<Trash2Icon />}
          >
            {t('comments.delete')}
          </Button>
        )}
      </Stack>
      {confirmingDelete && (
        <ConfirmDialog
          confirmLabel={t('comments.delete')}
          destructive
          error={remove.error}
          isPending={remove.isPending}
          onClose={() => setConfirmingDelete(false)}
          onConfirm={() => remove.mutate()}
          title={t('comments.deleteTitle')}
        >
          {t('comments.deleteConfirm')}
        </ConfirmDialog>
      )}
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

/**
 * A record's comments, oldest first like a conversation, with earlier pages
 * above them and the composer below.
 */
export const RecordCommentsPanel = ({ recordId }: { recordId: string }) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const [composerVersion, setComposerVersion] = useState(0);
  const [notice, setNotice] = useState('');
  const session = useQuery(sessionOptions());
  const commentsOptions = recordCommentsOptions(
    session.data ?? undefined,
    recordId,
  );
  const key = commentsOptions.queryKey;
  const comments = useInfiniteQuery(commentsOptions);
  const saved = () => {
    setNotice(t('comments.saved'));
    void client.invalidateQueries({ queryKey: key });
  };
  // Pages arrive newest first; the oldest loaded comment leads.
  const items = (comments.data?.pages.flatMap((page) => page.items) ?? [])
    .slice()
    .reverse();
  const sectionRef = useRef<HTMLDivElement>(null);
  // The section loads after the record, too late for the router's own
  // scroll to a link's #comments target.
  useEffect(() => {
    if (window.location.hash === `#${commentsSectionId}`)
      sectionRef.current?.scrollIntoView();
  }, []);
  return (
    <Box id={commentsSectionId} ref={sectionRef}>
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
      <NextPageButton
        compact
        label={t('comments.loadEarlier')}
        query={comments}
      />
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
            onDeleted={() => {
              setNotice(t('comments.deleted'));
              void client.invalidateQueries({ queryKey: key });
            }}
          />
        ))}
      </Box>
      {comments.isSuccess && items.length === 0 && (
        <EmptyState icon={CommentIcon} title={t('comments.empty')} />
      )}
      {session.data && (
        <Box sx={{ mt: 6 }}>
          <CommentComposer
            key={`${session.data.workspace_id}:${session.data.user_id}:${recordId}:${composerVersion}`}
            recordId={recordId}
            disabled={!comments.data || comments.isError}
            onSaved={() => {
              setComposerVersion((version) => version + 1);
              saved();
            }}
          />
        </Box>
      )}
    </Box>
  );
};
