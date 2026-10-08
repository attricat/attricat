import { Badge } from '@mui/material';
import { InboxIcon } from '../../components/systemIcons';
import { maxBadgeCount } from './constants';

/** The inbox icon with the unread count; the count is announced by the label. */
export const InboxNavigationIcon = ({ unread }: { unread: number }) => (
  <Badge
    aria-hidden
    badgeContent={unread}
    color="primary"
    max={maxBadgeCount}
    overlap="circular"
  >
    <InboxIcon />
  </Badge>
);
