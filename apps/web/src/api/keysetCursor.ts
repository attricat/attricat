/**
 * Where a newest-first list continues: after the item with this time and id.
 * Records' approvals and retention holds, comments and notifications page
 * this way.
 */
export type KeysetCursor = { before_time: string; before_id: string };

/** The query string that asks for the page after `cursor`. */
export const keysetQuery = (cursor?: KeysetCursor) =>
  cursor ? `?${new URLSearchParams(cursor)}` : '';

/** The cursor after a page's last item, or `undefined` when none follow. */
export const nextKeysetCursor = <T>(
  page: { items: readonly T[]; has_more: boolean },
  cursorOf: (item: T) => KeysetCursor,
) => {
  const last = page.items.at(-1);
  return page.has_more && last ? cursorOf(last) : undefined;
};

/** The cursor of an item listed by creation time. */
export const createdAtCursor = (item: {
  created_at: string;
  id: string;
}): KeysetCursor => ({ before_time: item.created_at, before_id: item.id });
