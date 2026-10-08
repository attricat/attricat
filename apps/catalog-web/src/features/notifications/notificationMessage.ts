import type { TFunction } from 'i18next';
import { lexiconText } from '../lexicon/lexicon';
import { notificationKinds } from './constants';
import type { Notification } from './schemas';

const dataText = (notification: Notification, key: string) => {
  const value = notification.data[key];
  return typeof value === 'string' && value.trim() ? value : undefined;
};

/**
 * The translated one-line message of a notification. `recordLabel` is the
 * subject record's current label when the reader may see it; otherwise the
 * record is named by its blueprint. Unknown kinds fall back to the title the
 * server stored.
 */
export const notificationMessage = (
  t: TFunction,
  notification: Notification,
  recordLabel?: string,
): string => {
  const actor =
    notification.actor_display_name ||
    notification.actor_email ||
    t('inbox.automation');
  const blueprint = dataText(notification, 'blueprint_name');
  const record = recordLabel
    ? t('inbox.namedRecord', { label: recordLabel })
    : blueprint
      ? t('inbox.blueprintRecord', { blueprint: lexiconText(blueprint) })
      : t('inbox.record');
  const team = dataText(notification, 'team_name');
  const conversation =
    dataText(notification, 'conversation_title') ??
    t('agents.untitledConversation');
  switch (notification.kind) {
    case notificationKinds.entityAssigned:
      return team
        ? t('inbox.messages.teamAssigned', { actor, record, team })
        : t('inbox.messages.assigned', { actor, record });
    case notificationKinds.entityCommented:
      return t('inbox.messages.commented', { actor, record });
    case notificationKinds.agentApprovalRequired:
      return t('inbox.messages.approvalRequired', { conversation });
    case notificationKinds.agentRunCompleted:
      return t('inbox.messages.runCompleted', { conversation });
    case notificationKinds.agentRunFailed:
      return t('inbox.messages.runFailed', { conversation });
    case notificationKinds.teamMemberAdded:
      return team
        ? t('inbox.messages.teamMemberAdded', { actor, team })
        : notification.title;
    case notificationKinds.invitationAccepted:
      return t('inbox.messages.invitationAccepted', { actor });
    default:
      return notification.title;
  }
};
