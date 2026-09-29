import { maximumTitleLength } from './constants';

export const conversationTitleFromFirstMessage = (content: string) => {
  const normalized = content.replace(/\s+/g, ' ').trim();
  if (normalized.length <= maximumTitleLength) return normalized;
  return `${normalized.slice(0, maximumTitleLength - 1)}…`;
};
