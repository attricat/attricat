import { principalKinds } from '../principals/constants';
import {
  parsePrincipalReference,
  principalReference,
  userLabel,
} from '../principals/principal';
import type { DirectoryUser } from '../principals/schemas';

/**
 * Mentions are Markdown links to a user reference, `[@Ada](user:<uuid>)`.
 * The server notifies every user a saved comment newly mentions.
 */
export const mentionTrigger = '@';

/** Characters typed after `@` that still search the directory. */
const maxMentionQueryLength = 50;

/**
 * An `@` at the start of the text or after whitespace, followed by the name
 * typed so far up to the caret. Names can contain spaces; a line break or
 * Markdown link punctuation ends the search.
 */
const mentionQueryPattern = new RegExp(
  `(?:^|\\s)${mentionTrigger}([^\\n${mentionTrigger}[\\]()]{0,${maxMentionQueryLength}})$`,
  'u',
);

/** Characters that would otherwise format or end a mention's link text. */
const linkTextSpecialCharacters = /[\\[\]*_`~<>]/g;

/** The mention being typed: where its `@` is and the text after it. */
export type MentionQuery = { start: number; query: string };

export const activeMentionQuery = (
  text: string,
  caret: number,
): MentionQuery | undefined => {
  const match = mentionQueryPattern.exec(text.slice(0, caret));
  if (!match) return undefined;
  const query = match[1];
  return { start: caret - query.length - mentionTrigger.length, query };
};

/**
 * Active users whose name or email contains the query, those whose name or
 * email starts with it first.
 */
export const mentionSuggestions = (
  users: DirectoryUser[],
  query: string,
  limit: number,
): DirectoryUser[] => {
  const needle = query.trim().toLocaleLowerCase();
  const fields = (user: DirectoryUser) =>
    [userLabel(user), user.email].map((field) => field.toLocaleLowerCase());
  return users
    .filter(
      (user) =>
        user.active && fields(user).some((field) => field.includes(needle)),
    )
    .map((user) => ({
      user,
      prefix: fields(user).some((field) => field.startsWith(needle)),
    }))
    .sort((left, right) => Number(right.prefix) - Number(left.prefix))
    .slice(0, limit)
    .map(({ user }) => user);
};

export const mentionMarkdown = (user: DirectoryUser) =>
  `[${mentionTrigger}${userLabel(user).replace(linkTextSpecialCharacters, '\\$&')}](${principalReference(principalKinds.user, user.id)})`;

/** Replaces the typed `@query` with a mention of `user` and a space. */
export const insertMention = (
  text: string,
  mention: MentionQuery,
  user: DirectoryUser,
): { text: string; cursor: number } => {
  const inserted = `${mentionMarkdown(user)} `;
  const end = mention.start + mentionTrigger.length + mention.query.length;
  return {
    text: text.slice(0, mention.start) + inserted + text.slice(end),
    cursor: mention.start + inserted.length,
  };
};

/** The user a Markdown link mentions, if its target is a user reference. */
export const mentionedUserId = (href: string | undefined) => {
  const reference = parsePrincipalReference(href);
  return reference?.kind === principalKinds.user ? reference.id : undefined;
};
