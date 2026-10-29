export const notificationsPath = '/api/notifications';

/** How often the navigation refreshes the unread count. */
export const unreadCountRefreshMs = 30_000;

/** The navigation badge shows this count at most, then `99+`. */
export const maxBadgeCount = 99;

export const recordRoute = '/records/$recordId';

/** What a notification is about, linking to it. */
export const notificationSubjectKinds = {
  record: 'record',
  agentConversation: 'agent_conversation',
  rule: 'rule',
} as const;

export const inboxFilters = { all: 'all', unread: 'unread' } as const;
export type InboxFilter = (typeof inboxFilters)[keyof typeof inboxFilters];

/** Notification kinds with a translated message; others show their title. */
export const notificationKinds = {
  recordAssigned: 'record.assigned',
  recordCommented: 'record.commented',
  recordMentioned: 'record.mentioned',
  agentApprovalRequired: 'agent.approval_required',
  agentRunCompleted: 'agent.run_completed',
  agentRunFailed: 'agent.run_failed',
  teamMemberAdded: 'team.member_added',
  invitationAccepted: 'workspace.invitation_accepted',
  ruleNotApplicable: 'rule.not_applicable',
} as const;
