import type { LucideIcon } from 'lucide-react';
import { compactIconSize } from '../../components/iconSizes';
import {
  AgentIcon,
  AssignedUserIcon,
  CommentIcon,
  InboxIcon,
  InvitationIcon,
  TeamIcon,
} from '../../components/systemIcons';
import { notificationKinds } from './constants';
import type { Notification } from './schemas';

const kindIcons: Record<string, LucideIcon> = {
  [notificationKinds.entityAssigned]: AssignedUserIcon,
  [notificationKinds.entityCommented]: CommentIcon,
  [notificationKinds.agentApprovalRequired]: AgentIcon,
  [notificationKinds.agentRunCompleted]: AgentIcon,
  [notificationKinds.agentRunFailed]: AgentIcon,
  [notificationKinds.teamMemberAdded]: TeamIcon,
  [notificationKinds.invitationAccepted]: InvitationIcon,
};

/** The decorative concept icon of what a notification is about. */
export const NotificationKindIcon = ({
  notification,
}: {
  notification: Notification;
}) => {
  const Icon =
    notification.kind === notificationKinds.entityAssigned &&
    notification.data.team_name
      ? TeamIcon
      : (kindIcons[notification.kind] ?? InboxIcon);
  return <Icon aria-hidden size={compactIconSize} />;
};
