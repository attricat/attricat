import { useTranslation } from 'react-i18next';
import { InboxNavigationIcon } from '../features/notifications/InboxNavigationIcon';
import { useUnreadNotificationCount } from '../features/notifications/useUnreadNotificationCount';
import { inboxNavigationItem } from './navigation';
import { NavigationItem } from './NavigationItem';
import { isWithinRoute } from './sideNavigationLayout';

/** The inbox link at the start of the navigation footer, with its unread count. */
export const InboxNavigationItem = ({
  compact,
  onClick,
  pathname,
}: {
  compact: boolean;
  onClick?: () => void;
  pathname: string;
}) => {
  const { t } = useTranslation();
  const unread = useUnreadNotificationCount();
  return (
    <NavigationItem
      accessibleLabel={
        unread > 0 ? t('navigation.inboxUnread', { count: unread }) : undefined
      }
      compact={compact}
      icon={<InboxNavigationIcon unread={unread} />}
      label={t(inboxNavigationItem.labelKey)}
      onClick={onClick}
      selected={isWithinRoute(pathname, inboxNavigationItem.to)}
      to={inboxNavigationItem.to}
    />
  );
};
