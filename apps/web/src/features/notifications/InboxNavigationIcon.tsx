import { Badge } from '@mui/material';
import { NavigationIcon } from '../../components/NavigationIcon';
import { InboxIcon } from '../../components/systemIcons';
import { inboxBadgeOverlapPx, maxBadgeCount } from './constants';

/** The inbox icon with the unread count; the count is announced by the label. */
export const InboxNavigationIcon = ({ unread }: { unread: number }) => (
  <Badge
    aria-hidden
    badgeContent={unread}
    color="primary"
    max={maxBadgeCount}
    overlap="circular"
    sx={{
      // A wide count grows away from the icon instead of covering it.
      '& .MuiBadge-badge:not(.MuiBadge-invisible)': {
        transform: `translate(calc(100% - ${inboxBadgeOverlapPx}px), -50%)`,
      },
    }}
  >
    <NavigationIcon icon={InboxIcon} />
  </Badge>
);
