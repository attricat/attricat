import { useInfiniteQuery } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Box, Button, List, ListItem, Paper, Tab, Tabs } from '@mui/material';
import { CheckCheckIcon } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { NextPageButton } from '../../components/LoadMoreButton';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { QueryErrorNotice } from '../../components/QueryErrorNotice';
import { EmptyState } from '../../components/EmptyState';
import { InboxIcon } from '../../components/systemIcons';
import { useRecordLabels } from '../records/useRecordLabels';
import { listNotifications } from './api';
import { navigationRoutes } from '../../components/navigation';
import {
  inboxFilters,
  notificationSubjectKinds,
  type InboxFilter,
} from './constants';
import { NotificationListItem } from './NotificationListItem';
import { notificationQueryKeys } from './queryKeys';
import {
  createdAtCursor,
  nextKeysetCursor,
  type KeysetCursor,
} from '../../api/keysetCursor';
import { useInboxActions } from './useInboxActions';
import { useInboxIdentity } from './useInboxIdentity';
import { LoadingState } from '../../components/LoadingState';

const tabId = (filter: InboxFilter) => `inbox-tab-${filter}`;
const panelId = 'inbox-panel';

export const InboxPage = ({ filter }: { filter: InboxFilter }) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { session, workspaceId, userId } = useInboxIdentity();
  const notifications = useInfiniteQuery({
    queryKey: notificationQueryKeys.list(workspaceId, userId, filter),
    queryFn: ({ pageParam, signal }) =>
      listNotifications(filter === inboxFilters.unread, pageParam, signal),
    initialPageParam: undefined as KeysetCursor | undefined,
    getNextPageParam: (page) => nextKeysetCursor(page, createdAtCursor),
    enabled: Boolean(workspaceId && userId),
  });
  const { isBusy, markAllRead, remove, setRead } = useInboxActions(
    workspaceId,
    userId,
  );
  const pages = notifications.data?.pages ?? [];
  const items = pages.flatMap((page) => page.items);
  // The newest page reports the current count; later pages can be older.
  const unreadCount = pages[0]?.unread_count ?? 0;
  const recordLabels = useRecordLabels(
    items.flatMap((item) =>
      item.subject?.kind === notificationSubjectKinds.record
        ? [item.subject.id]
        : [],
    ),
  );
  const error = notifications.error ?? session.error;
  // Marking all read disables the button, which drops keyboard focus. While
  // the button keeps focus it holds it; once the refreshed count disables it,
  // focus moves to the selected tab.
  const markAllButton = useRef<HTMLButtonElement>(null);
  const restoreMarkAllFocus = useRef(false);
  useEffect(() => {
    if (markAllRead.isPending || !restoreMarkAllFocus.current) return;
    const button = markAllButton.current;
    const focused = document.activeElement;
    // Focus that moved elsewhere meanwhile stays where it is.
    if (focused && focused !== document.body && focused !== button) {
      restoreMarkAllFocus.current = false;
      return;
    }
    if (button && !button.disabled) {
      button.focus();
      return;
    }
    restoreMarkAllFocus.current = false;
    document.getElementById(tabId(filter))?.focus();
  }, [filter, markAllRead.isPending, unreadCount]);

  return (
    <PageContainer>
      <PageHeader
        actions={
          <Button
            disabled={unreadCount === 0 || markAllRead.isPending}
            onClick={(event) => {
              restoreMarkAllFocus.current =
                document.activeElement === event.currentTarget;
              // Notifications that arrive after the newest one shown stay
              // unread.
              markAllRead.mutate(items[0]?.created_at);
            }}
            ref={markAllButton}
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
              to: navigationRoutes.inbox,
              search: {
                filter: value === inboxFilters.all ? undefined : value,
              },
            })
          }
          value={filter}
        >
          {Object.values(inboxFilters).map((value) => (
            <Tab
              aria-controls={panelId}
              id={tabId(value)}
              key={value}
              label={
                value === inboxFilters.unread
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
                busy={isBusy(notification.id)}
                key={notification.id}
                notification={notification}
                onDelete={() => remove.mutate(notification.id)}
                onOpen={() => {
                  if (!notification.read && !isBusy(notification.id))
                    setRead.mutate({ id: notification.id, read: true });
                }}
                onToggleRead={() =>
                  setRead.mutate({
                    id: notification.id,
                    read: !notification.read,
                  })
                }
                recordLabel={
                  notification.subject?.kind === notificationSubjectKinds.record
                    ? recordLabels.get(notification.subject.id)
                    : undefined
                }
              />
            ))}
            {notifications.isPending && !error && (
              <ListItem>
                <LoadingState>{t('inbox.loading')}</LoadingState>
              </ListItem>
            )}
            {notifications.isSuccess && items.length === 0 && (
              <ListItem sx={{ display: 'block' }}>
                <EmptyState
                  description={t('inbox.emptyDescription')}
                  icon={InboxIcon}
                  title={t(
                    filter === inboxFilters.unread
                      ? 'inbox.noUnread'
                      : 'inbox.empty',
                  )}
                />
              </ListItem>
            )}
          </List>
        </Paper>
        <NextPageButton query={notifications} sx={{ mt: 2 }} />
      </Box>
    </PageContainer>
  );
};
