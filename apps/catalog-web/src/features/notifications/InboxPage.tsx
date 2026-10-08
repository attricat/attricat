import { useInfiniteQuery } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import {
  Box,
  Button,
  List,
  ListItem,
  ListItemText,
  Paper,
  Tab,
  Tabs,
  Typography,
} from '@mui/material';
import { CheckCheckIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { LoadMoreButton } from '../../components/LoadMoreButton';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { QueryErrorNotice } from '../../components/QueryErrorNotice';
import { InboxIcon } from '../../components/systemIcons';
import { useEntityLabels } from '../entities/useEntityLabels';
import { listNotifications } from './api';
import { inboxFilters, inboxRoute, type InboxFilter } from './constants';
import { NotificationListItem } from './NotificationListItem';
import { notificationQueryKeys } from './queryKeys';
import type { NotificationCursor } from './schemas';
import { useInboxActions } from './useInboxActions';
import { useInboxIdentity } from './useInboxIdentity';

const tabId = (filter: InboxFilter) => `inbox-tab-${filter}`;
const panelId = 'inbox-panel';

export const InboxPage = ({ filter }: { filter: InboxFilter }) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { session, workspaceId, userId } = useInboxIdentity();
  const notifications = useInfiniteQuery({
    queryKey: notificationQueryKeys.list(workspaceId, userId, filter),
    queryFn: ({ pageParam, signal }) =>
      listNotifications(filter === 'unread', pageParam, signal),
    initialPageParam: undefined as NotificationCursor | undefined,
    getNextPageParam: (page) => {
      const last = page.items.at(-1);
      return page.has_more && last
        ? { before_time: last.created_at, before_id: last.id }
        : undefined;
    },
    enabled: Boolean(workspaceId && userId),
  });
  const { setRead, markAllRead, remove } = useInboxActions(workspaceId, userId);
  const pages = notifications.data?.pages ?? [];
  const items = pages.flatMap((page) => page.items);
  // The newest page reports the current count; later pages can be older.
  const unreadCount = pages[0]?.unread_count ?? 0;
  const recordLabels = useEntityLabels(
    items.flatMap((item) =>
      item.subject?.kind === 'entity' ? [item.subject.id] : [],
    ),
  );
  const busyId = setRead.isPending
    ? setRead.variables.id
    : remove.isPending
      ? remove.variables
      : undefined;
  const error = notifications.error ?? session.error;

  return (
    <PageContainer>
      <PageHeader
        actions={
          <Button
            disabled={unreadCount === 0 || markAllRead.isPending}
            onClick={() =>
              // Notifications that arrive after the newest one shown stay
              // unread.
              markAllRead.mutate(items[0]?.created_at)
            }
            startIcon={<CheckCheckIcon />}
            variant="outlined"
          >
            {t('inbox.markAllRead')}
          </Button>
        }
        description={t('inbox.description')}
        icon={InboxIcon}
        title={t('inbox.title')}
      />
      <Box sx={{ borderBottom: 1, borderColor: 'divider', mt: 3 }}>
        <Tabs
          aria-label={t('inbox.filter')}
          onChange={(_, value: InboxFilter) =>
            void navigate({
              to: inboxRoute,
              search: { filter: value === 'all' ? undefined : value },
            })
          }
          value={filter}
        >
          {inboxFilters.map((value) => (
            <Tab
              aria-controls={panelId}
              id={tabId(value)}
              key={value}
              label={
                value === 'unread'
                  ? t('inbox.filters.unreadCount', { count: unreadCount })
                  : t('inbox.filters.all')
              }
              value={value}
            />
          ))}
        </Tabs>
      </Box>
      <Box
        aria-labelledby={tabId(filter)}
        id={panelId}
        role="tabpanel"
        sx={{ mt: 2 }}
      >
        <QueryErrorNotice
          error={error}
          isRetrying={notifications.isFetching || session.isFetching}
          onRetry={() => {
            void session.refetch();
            void notifications.refetch();
          }}
        />
        <Paper sx={{ mt: error ? 2 : 0 }}>
          <List disablePadding>
            {items.map((notification) => (
              <NotificationListItem
                busy={busyId === notification.id}
                key={notification.id}
                notification={notification}
                onDelete={() => remove.mutate(notification.id)}
                onOpen={() => {
                  if (!notification.read)
                    setRead.mutate({ id: notification.id, read: true });
                }}
                onToggleRead={() =>
                  setRead.mutate({
                    id: notification.id,
                    read: !notification.read,
                  })
                }
                recordLabel={
                  notification.subject?.kind === 'entity'
                    ? recordLabels.get(notification.subject.id)
                    : undefined
                }
              />
            ))}
            {notifications.isPending && !error && (
              <ListItem>
                <Typography>{t('inbox.loading')}</Typography>
              </ListItem>
            )}
            {notifications.isSuccess && items.length === 0 && (
              <ListItem>
                <ListItemText
                  primary={t(
                    filter === 'unread' ? 'inbox.noUnread' : 'inbox.empty',
                  )}
                  secondary={t('inbox.emptyDescription')}
                />
              </ListItem>
            )}
          </List>
        </Paper>
        {notifications.hasNextPage && (
          <Box sx={{ mt: 2 }}>
            <LoadMoreButton
              isLoading={notifications.isFetchingNextPage}
              onLoadMore={() => void notifications.fetchNextPage()}
            />
          </Box>
        )}
      </Box>
    </PageContainer>
  );
};
