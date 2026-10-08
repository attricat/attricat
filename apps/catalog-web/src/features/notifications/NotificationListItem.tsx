import {
  Box,
  IconButton,
  ListItem,
  Stack,
  Tooltip,
  Typography,
} from '@mui/material';
import { MailIcon, MailOpenIcon, Trash2Icon } from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import { useTranslation } from 'react-i18next';
import { RouterListItemButton } from '../../components/RouterLink';
import { Timestamp } from '../../time/Timestamp';
import { conversationRoute, entityRoute } from './constants';
import { NotificationKindIcon } from './NotificationKindIcon';
import { notificationMessage } from './notificationMessage';
import type { Notification } from './schemas';

type NotificationListItemProps = {
  busy: boolean;
  notification: Notification;
  onDelete: () => void;
  onOpen: () => void;
  onToggleRead: () => void;
  recordLabel?: string;
};

const contentSx = {
  alignItems: 'flex-start',
  display: 'flex',
  flex: 1,
  gap: 1.5,
  minWidth: 0,
  px: 2,
  py: 1.5,
} as const;

export const NotificationListItem = ({
  busy,
  notification,
  onDelete,
  onOpen,
  onToggleRead,
  recordLabel,
}: NotificationListItemProps) => {
  const { t } = useTranslation();
  const message = notificationMessage(t, notification, recordLabel);
  const unread = !notification.read;
  const subject = notification.subject;
  const toggleLabel = t(unread ? 'inbox.markRead' : 'inbox.markUnread');
  const content = (
    <>
      <Box
        aria-hidden={!unread}
        aria-label={unread ? t('inbox.unread') : undefined}
        component="span"
        role={unread ? 'img' : undefined}
        sx={{
          bgcolor: unread ? 'primary.main' : 'transparent',
          borderRadius: '50%',
          flexShrink: 0,
          height: 8,
          // Centre the dot on the first line of the message.
          mt: 1.25,
          width: 8,
        }}
      />
      <Box
        component="span"
        sx={{
          color: 'text.secondary',
          display: 'inline-flex',
          flexShrink: 0,
          // Centre the icon on the first line of the message.
          mt: 0.75,
        }}
      >
        <NotificationKindIcon notification={notification} />
      </Box>
      <Stack component="span" spacing={0.5} sx={{ minWidth: 0 }}>
        <Typography
          component="span"
          sx={{ fontWeight: unread ? 600 : 400, overflowWrap: 'anywhere' }}
        >
          {message}
        </Typography>
        {notification.body && (
          // Comment excerpts are Markdown; show their text only, since the
          // row is already a link.
          <Typography
            color="text.secondary"
            component="span"
            sx={{
              display: '-webkit-box',
              overflow: 'hidden',
              overflowWrap: 'anywhere',
              WebkitBoxOrient: 'vertical',
              WebkitLineClamp: 2,
            }}
            variant="body2"
          >
            <ReactMarkdown allowedElements={[]} skipHtml unwrapDisallowed>
              {notification.body}
            </ReactMarkdown>
          </Typography>
        )}
        <Typography color="text.secondary" component="span" variant="caption">
          <Timestamp focusable={false} value={notification.created_at} />
        </Typography>
      </Stack>
    </>
  );
  return (
    <ListItem disablePadding divider sx={{ alignItems: 'flex-start' }}>
      {subject?.kind === 'entity' ? (
        <RouterListItemButton
          onClick={onOpen}
          params={{ entityId: subject.id }}
          sx={contentSx}
          to={entityRoute}
        >
          {content}
        </RouterListItemButton>
      ) : subject?.kind === 'agent_conversation' ? (
        <RouterListItemButton
          onClick={onOpen}
          params={{ conversationId: subject.id }}
          sx={contentSx}
          to={conversationRoute}
        >
          {content}
        </RouterListItemButton>
      ) : (
        <Box sx={contentSx}>{content}</Box>
      )}
      <Stack direction="row" sx={{ flexShrink: 0, pr: 1, py: 1 }}>
        <Tooltip title={toggleLabel}>
          <span>
            <IconButton
              aria-label={toggleLabel}
              disabled={busy}
              onClick={onToggleRead}
            >
              {unread ? <MailOpenIcon /> : <MailIcon />}
            </IconButton>
          </span>
        </Tooltip>
        <Tooltip title={t('inbox.delete')}>
          <span>
            <IconButton
              aria-label={t('inbox.delete')}
              disabled={busy}
              onClick={onDelete}
            >
              <Trash2Icon />
            </IconButton>
          </span>
        </Tooltip>
      </Stack>
    </ListItem>
  );
};
