export const maxCommentLength = 10_000;
export const commentComposerMaxWidth = 720;
/** Anchor of the comments section on the record page. */
export const commentsSectionId = 'comments';
/**
 * Author kind of comments written by the workspace's system user, such as
 * the note left on a record a bulk edit failed to change.
 */
export const systemAuthorKind = 'system';
export const authorKinds = ['person', systemAuthorKind] as const;
/** Users the mention autocomplete lists at once. */
export const maxMentionSuggestions = 8;
// The mention list shows about eight users before scrolling.
export const mentionSuggestionListMaxHeight = 360;
