import { isJsonObject as isRecord } from '../../api/jsonObject';
import { messageRoles } from './constants';
import type { ConversationMessage } from './schemas';

/** The tool calls an assistant message requests, or none for a reply. */
export const requestedCalls = (message: ConversationMessage): unknown[] =>
  message.role === messageRoles.assistant &&
  isRecord(message.content) &&
  Array.isArray(message.content.tool_calls)
    ? message.content.tool_calls
    : [];

export const callName = (call: unknown) =>
  isRecord(call) &&
  isRecord(call.function) &&
  typeof call.function.name === 'string'
    ? call.function.name
    : null;

/** Tool calls and their results, as opposed to text for the user. */
const isToolActivity = (message: ConversationMessage) =>
  message.role === messageRoles.tool || requestedCalls(message).length > 0;

/** Messages in order, with each run of tool activity gathered into one list. */
export const groupToolActivity = (messages: ConversationMessage[]) =>
  messages.reduce<(ConversationMessage | ConversationMessage[])[]>(
    (groups, message) => {
      const last = groups.at(-1);
      if (!isToolActivity(message)) groups.push(message);
      else if (Array.isArray(last)) last.push(message);
      else groups.push([message]);
      return groups;
    },
    [],
  );
