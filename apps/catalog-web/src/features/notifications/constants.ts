export const notificationsPath = '/api/notifications';

/** How often the navigation refreshes the unread count. */
export const unreadCountRefreshMs = 30_000;

/** The navigation badge shows this count at most, then `99+`. */
export const maxBadgeCount = 99;

export const inboxRoute = '/inbox';
export const entityRoute = '/entities/$entityId';
export const conversationRoute = '/agents/$conversationId';

export const inboxFilters = ['all', 'unread'] as const;
export type InboxFilter = (typeof inboxFilters)[number];

/** Notification kinds with a translated message; others show their title. */
export const notificationKinds = {
  entityAssigned: 'entity.assigned',
  entityCommented: 'entity.commented',
  agentApprovalRequired: 'agent.approval_required',
  agentRunCompleted: 'agent.run_completed',
  agentRunFailed: 'agent.run_failed',
  teamMemberAdded: 'team.member_added',
  invitationAccepted: 'workspace.invitation_accepted',
} as const;
