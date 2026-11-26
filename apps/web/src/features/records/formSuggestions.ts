import type { Attribute } from './api';
import { valueForField } from './attributeValues';

/** What put a value into an unsaved form field. */
export const suggestionSources = {
  /** The attribute's `default_from`, which the save applies too. */
  defaultFrom: 'default_from',
  /** An extension's `form_assist` proposal, applied only by the form. */
  assist: 'assist',
} as const;
export type SuggestionSource =
  (typeof suggestionSources)[keyof typeof suggestionSources];

/** A value the form filled in for the user, until they edit the field. */
export type FormSuggestion = {
  /** The field text the suggestion set. */
  value: string;
  source: SuggestionSource;
  /** The related record a `default_from` value came from. */
  recordId?: string;
  /** An assist's hint shown under the field. */
  note?: string;
};

export type FormSuggestions = Readonly<Record<string, FormSuggestion>>;

/** A proposed field value, as JSON from the server or an extension. */
export type FieldProposal = {
  attributeCode: string;
  value: Parameters<typeof valueForField>[0];
  recordId?: string;
  note?: string;
};

/**
 * Sources whose suggestion another source may replace: an assist may replace
 * a `default_from` value (it applies after), never the other way round.
 */
const replaceableBy: Readonly<
  Record<SuggestionSource, readonly SuggestionSource[]>
> = {
  [suggestionSources.defaultFrom]: [suggestionSources.defaultFrom],
  [suggestionSources.assist]: [
    suggestionSources.defaultFrom,
    suggestionSources.assist,
  ],
};

/**
 * Whether `source` may set field `code`: the user has not edited it, and it
 * is empty or still holds a suggestion that source may replace.
 */
export const maySuggest = (
  code: string,
  source: SuggestionSource,
  fields: Readonly<Record<string, string>>,
  suggestions: FormSuggestions,
  edited: ReadonlySet<string>,
): boolean => {
  if (edited.has(code)) return false;
  const current = fields[code] ?? '';
  const suggestion = suggestions[code];
  if (!current.trim()) return true;
  return (
    suggestion !== undefined &&
    suggestion.value === current &&
    replaceableBy[source].includes(suggestion.source)
  );
};

/**
 * Applies `proposals` from `source` to the unsaved form. Fields in `scope`
 * that still hold an earlier suggestion of the same source but get no new
 * proposal are emptied, since that suggestion came from a choice that no
 * longer holds (such as another item). Returns the new fields and
 * suggestions, or `undefined` when nothing changes.
 */
export const applySuggestions = ({
  edited,
  fields,
  proposals,
  scope,
  source,
  suggestions,
}: {
  edited: ReadonlySet<string>;
  fields: Readonly<Record<string, string>>;
  proposals: readonly FieldProposal[];
  /** The fields this source may propose for. */
  scope: readonly string[];
  source: SuggestionSource;
  suggestions: FormSuggestions;
}):
  | { fields: Record<string, string>; suggestions: FormSuggestions }
  | undefined => {
  const nextFields = { ...fields };
  const nextSuggestions: Record<string, FormSuggestion> = { ...suggestions };
  let changed = false;
  const proposed = new Map(
    proposals.map((proposal) => [proposal.attributeCode, proposal]),
  );
  for (const code of scope) {
    const proposal = proposed.get(code);
    if (!maySuggest(code, source, fields, suggestions, edited)) continue;
    if (!proposal) {
      const stale = suggestions[code];
      if (stale?.source === source && stale.value === (fields[code] ?? '')) {
        nextFields[code] = '';
        delete nextSuggestions[code];
        changed = true;
      }
      continue;
    }
    const value = valueForField(proposal.value);
    nextFields[code] = value;
    nextSuggestions[code] = {
      value,
      source,
      ...(proposal.recordId === undefined
        ? {}
        : { recordId: proposal.recordId }),
      ...(proposal.note === undefined ? {} : { note: proposal.note }),
    };
    changed = changed || value !== (fields[code] ?? '');
    changed = changed || suggestions[code]?.source !== source;
  }
  return changed
    ? { fields: nextFields, suggestions: nextSuggestions }
    : undefined;
};

/** Forgets the suggestions of fields the user has now edited. */
export const withoutEdited = (
  suggestions: FormSuggestions,
  edited: Iterable<string>,
): FormSuggestions => {
  const next: Record<string, FormSuggestion> = { ...suggestions };
  let changed = false;
  for (const code of edited) {
    if (code in next) {
      delete next[code];
      changed = true;
    }
  }
  return changed ? next : suggestions;
};

/** The relationship a `default_from` path starts with. */
export const defaultFromRelationship = (attribute: Attribute) =>
  attribute.default_from?.split('.')[0];

/** Attributes whose `default_from` starts at `relationship`. */
export const defaultFromDependents = (
  attributes: readonly Attribute[],
  relationship: string,
): Attribute[] =>
  attributes.filter(
    (attribute) => defaultFromRelationship(attribute) === relationship,
  );
