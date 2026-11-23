import { z } from 'zod';
import type { RecordItem } from '../records/api';
import { recordItemSchema } from '../records/schemas';
import { MAX_RELATIONSHIP_PICKER_SELECTION } from '../records/components/useRecentlyPreviewedRecords';

/**
 * What a relationship picker tab chose and created, kept in its session
 * storage while it moves between the Explorer and the create form. Entries
 * are keyed by the picker's token and live only as long as the tab.
 */
const selectionKey = (token: string) =>
  `attricat.relationshipPicker.${token}.selection`;
const createdKey = (token: string) =>
  `attricat.relationshipPicker.${token}.created`;

const read = <T>(key: string, schema: z.ZodType<T>, fallback: T): T => {
  try {
    const stored = window.sessionStorage.getItem(key);
    if (stored === null) return fallback;
    const parsed = schema.safeParse(JSON.parse(stored));
    return parsed.success ? parsed.data : fallback;
  } catch {
    return fallback;
  }
};

const write = (key: string, value: unknown) => {
  try {
    window.sessionStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Storage can be full or blocked; the selection then lasts for the page.
  }
};

const storedSelectionSchema = z
  .array(recordItemSchema)
  .max(MAX_RELATIONSHIP_PICKER_SELECTION);
const createdSchema = z.array(z.uuid()).max(MAX_RELATIONSHIP_PICKER_SELECTION);

/** The records a picker tab has selected, as it left the Explorer. */
export const readPickerSelection = (token: string): RecordItem[] =>
  read(selectionKey(token), storedSelectionSchema, []);

export const writePickerSelection = (token: string, records: RecordItem[]) =>
  write(selectionKey(token), records);

/** Remembers a record created for the picker, to select it in the Explorer. */
export const addPickerCreatedRecord = (token: string, recordId: string) => {
  const created = read(createdKey(token), createdSchema, []);
  if (created.includes(recordId)) return;
  write(
    createdKey(token),
    [...created, recordId].slice(-MAX_RELATIONSHIP_PICKER_SELECTION),
  );
};

/** The records created for the picker that the Explorer has not added yet. */
export const readPickerCreatedRecords = (token: string): string[] =>
  read(createdKey(token), createdSchema, []);

/** Forgets created records once the Explorer added them to its selection. */
export const forgetPickerCreatedRecords = (token: string, ids: string[]) => {
  const remaining = readPickerCreatedRecords(token).filter(
    (id) => !ids.includes(id),
  );
  try {
    if (remaining.length === 0)
      window.sessionStorage.removeItem(createdKey(token));
    else write(createdKey(token), remaining);
  } catch {
    // See write.
  }
};
