import type { Attribute } from '../entities/api';
import { attributeValueTypes } from '../entities/valueTypes';
import { relationshipPathSeparator } from './constants';

// Explorer query syntax; see apps/docs/src/content/docs/guides/search-syntax.md.
export const querySelectorSeparator = ':';
export const queryWildcard = '*';
/** A selector may name at most three relationships and a scalar leaf. */
export const maximumQueryRelationshipHops = 3;

export type QueryToken = { text: string; start: number; end: number };

export type QuerySegmentKind =
  'field' | 'global' | 'punctuation' | 'value' | 'wildcard' | 'whitespace';

export type QuerySegment = {
  kind: QuerySegmentKind;
  text: string;
  start: number;
  /** Set when this segment makes the query invalid. */
  error?: QueryError;
};

export type QueryError =
  | { code: 'malformedTerm'; term: string }
  | { code: 'misplacedWildcard'; term: string }
  | { code: 'tooManyHops' }
  | { code: 'unknownField'; field: string }
  | { code: 'unknownRelationship'; field: string }
  | { code: 'notSearchableLeaf'; field: string };

/** Attribute schemas of the selected blueprint and of relationship targets. */
export type QuerySchema = {
  blueprint: { code: string; name: string };
  attributes: Attribute[];
  /** Attributes of other blueprints by code; missing codes are still loading. */
  targets: Map<string, Attribute[]>;
};

const isRelationship = (attribute: Attribute) =>
  attribute.value_type === attributeValueTypes.relationship;

/** Types a relationship path cannot end on (mirrors the server's leaf rule). */
const unsearchableLeafTypes: string[] = [
  attributeValueTypes.relationship,
  attributeValueTypes.file,
  attributeValueTypes.json,
];

export const isSearchableLeaf = (attribute: Attribute) =>
  !unsearchableLeafTypes.includes(attribute.value_type);

const sameCode = (left: string, right: string) =>
  left.toLowerCase() === right.toLowerCase();

export const findAttribute = (
  attributes: Attribute[],
  code: string,
  predicate: (attribute: Attribute) => boolean,
) =>
  attributes.find(
    (attribute) => sameCode(attribute.code, code) && predicate(attribute),
  );

export const isBlueprintAlias = (schema: QuerySchema, name: string) =>
  sameCode(name, schema.blueprint.code) ||
  sameCode(name, schema.blueprint.name);

export const tokenizeQuery = (query: string): QueryToken[] =>
  [...query.matchAll(/\S+/g)].map((match) => ({
    text: match[0],
    start: match.index,
    end: match.index + match[0].length,
  }));

type Resolution =
  | { status: 'ok' }
  | { status: 'pending' }
  | { status: 'error'; error: QueryError; part: number };

/**
 * Walks a relationship path, returning the attributes of the blueprint the
 * path ends on, `undefined` while a target is loading, or the index of the
 * first unknown relationship.
 */
export const resolveRelationshipPath = (
  schema: QuerySchema,
  path: string[],
):
  | { status: 'ok'; attributes: Attribute[]; targetCode?: string }
  | { status: 'pending' }
  | { status: 'error'; part: number } => {
  let attributes = schema.attributes;
  let targetCode: string | undefined;
  for (const [index, name] of path.entries()) {
    const relationship = findAttribute(attributes, name, isRelationship);
    if (!relationship?.target_blueprint_code) {
      return { status: 'error', part: index };
    }
    targetCode = relationship.target_blueprint_code;
    const target = schema.targets.get(targetCode);
    if (!target) return { status: 'pending' };
    attributes = target;
  }
  return { status: 'ok', attributes, targetCode };
};

/** Mirrors `compile_search_term` in the API's entity search repository. */
export const resolveSelector = (
  schema: QuerySchema,
  parts: string[],
): Resolution => {
  if (parts.length === 1 && parts[0] === queryWildcard) return { status: 'ok' };
  if (parts.length > maximumQueryRelationshipHops + 1) {
    return {
      status: 'error',
      error: { code: 'tooManyHops' },
      part: maximumQueryRelationshipHops + 1,
    };
  }
  const empty = parts.findIndex((part) => !part);
  if (empty >= 0) {
    return {
      status: 'error',
      error: { code: 'malformedTerm', term: parts.join('.') },
      part: empty,
    };
  }
  const [first] = parts;
  if (parts.length === 1 && isBlueprintAlias(schema, first)) {
    return { status: 'ok' };
  }
  if (parts.length === 2 && isBlueprintAlias(schema, first)) {
    return findAttribute(
      schema.attributes,
      parts[1],
      (attribute) => !isRelationship(attribute),
    )
      ? { status: 'ok' }
      : {
          status: 'error',
          error: { code: 'unknownField', field: parts[1] },
          part: 1,
        };
  }
  if (parts.length === 1) {
    return findAttribute(schema.attributes, first, () => true)
      ? { status: 'ok' }
      : {
          status: 'error',
          error: { code: 'unknownField', field: first },
          part: 0,
        };
  }
  const path = resolveRelationshipPath(schema, parts.slice(0, -1));
  if (path.status === 'pending') return path;
  if (path.status === 'error') {
    return {
      status: 'error',
      error: { code: 'unknownRelationship', field: parts[path.part] },
      part: path.part,
    };
  }
  const leaf = parts[parts.length - 1];
  const leafAttribute = findAttribute(path.attributes, leaf, () => true);
  if (leafAttribute && isSearchableLeaf(leafAttribute)) return { status: 'ok' };
  return {
    status: 'error',
    error: leafAttribute
      ? { code: 'notSearchableLeaf', field: leaf }
      : { code: 'unknownField', field: leaf },
    part: parts.length - 1,
  };
};

const valueSegments = (
  value: string,
  start: number,
  term: string,
): QuerySegment[] => {
  const wildcard = value.endsWith(queryWildcard);
  const body = wildcard ? value.slice(0, -1) : value;
  const misplaced =
    body.includes(queryWildcard) || (wildcard && !body)
      ? ({ code: 'misplacedWildcard', term } as const)
      : undefined;
  const malformed = body.includes(querySelectorSeparator)
    ? ({ code: 'malformedTerm', term } as const)
    : undefined;
  const error = malformed ?? misplaced;
  return [
    ...(body ? [{ kind: 'value' as const, text: body, start, error }] : []),
    ...(wildcard
      ? [
          {
            kind: 'wildcard' as const,
            text: queryWildcard,
            start: start + body.length,
            error,
          },
        ]
      : []),
  ];
};

const selectorSegments = (
  schema: QuerySchema | undefined,
  selector: string,
  start: number,
): QuerySegment[] => {
  if (selector === queryWildcard) {
    return [{ kind: 'global', text: selector, start }];
  }
  const parts = selector.split(relationshipPathSeparator);
  const resolution = schema ? resolveSelector(schema, parts) : undefined;
  const segments: QuerySegment[] = [];
  let offset = start;
  for (const [index, part] of parts.entries()) {
    if (index > 0) {
      segments.push({
        kind: 'punctuation',
        text: relationshipPathSeparator,
        start: offset,
      });
      offset += relationshipPathSeparator.length;
    }
    if (part) {
      const error =
        resolution?.status === 'error' &&
        (resolution.part === index ||
          (resolution.error.code === 'tooManyHops' && index >= resolution.part))
          ? resolution.error
          : undefined;
      segments.push({ kind: 'field', text: part, start: offset, error });
    }
    offset += part.length;
  }
  // An empty path part has no text to underline, so flag the whole selector.
  if (resolution?.status === 'error' && !parts[resolution.part]) {
    return segments.map((segment) => ({ ...segment, error: resolution.error }));
  }
  return segments;
};

const termSegments = (
  schema: QuerySchema | undefined,
  token: QueryToken,
): QuerySegment[] => {
  const separator = token.text.indexOf(querySelectorSeparator);
  if (separator < 0) return valueSegments(token.text, token.start, token.text);
  const selector = token.text.slice(0, separator);
  const value = token.text.slice(separator + 1);
  const colon: QuerySegment = {
    kind: 'punctuation',
    text: querySelectorSeparator,
    start: token.start + separator,
  };
  const incomplete =
    !selector || !value
      ? ({ code: 'malformedTerm', term: token.text } as const)
      : undefined;
  return [
    ...selectorSegments(schema, selector, token.start),
    incomplete ? { ...colon, error: incomplete } : colon,
    ...valueSegments(value, token.start + separator + 1, token.text),
  ];
};

/**
 * Splits a query into highlighted segments that cover every character, so
 * the rendered overlay lines up with the text field it decorates.
 */
export const highlightQuery = (
  query: string,
  schema?: QuerySchema,
): QuerySegment[] => {
  const segments: QuerySegment[] = [];
  let offset = 0;
  for (const token of tokenizeQuery(query)) {
    if (token.start > offset) {
      segments.push({
        kind: 'whitespace',
        text: query.slice(offset, token.start),
        start: offset,
      });
    }
    segments.push(...termSegments(schema, token));
    offset = token.end;
  }
  if (offset < query.length) {
    segments.push({
      kind: 'whitespace',
      text: query.slice(offset),
      start: offset,
    });
  }
  return segments;
};

/**
 * Returns the first problem in the query, ignoring the term at `cursor`
 * while it is still incomplete (for example `sku:` before a value is typed).
 */
export const firstQueryError = (
  segments: QuerySegment[],
  query: string,
  cursor?: number,
) => {
  const editing = tokenizeQuery(query).find(
    (token) =>
      cursor !== undefined && cursor >= token.start && cursor <= token.end,
  );
  return segments.find(
    (segment) =>
      segment.error &&
      !(
        editing &&
        segment.error.code === 'malformedTerm' &&
        segment.start >= editing.start &&
        segment.start < editing.end
      ),
  )?.error;
};

export type QueryCursorContext =
  | {
      kind: 'field';
      /** Relationship codes already typed before the part being completed. */
      path: string[];
      partial: string;
      /** Query range the chosen suggestion replaces. */
      replaceStart: number;
      replaceEnd: number;
    }
  | {
      kind: 'value';
      selector: string;
      partial: string;
      replaceStart: number;
      replaceEnd: number;
    };

/** Describes what the user is typing at `cursor`, for autocomplete. */
export const queryCursorContext = (
  query: string,
  cursor: number,
): QueryCursorContext => {
  const token = tokenizeQuery(query).find(
    (candidate) => cursor >= candidate.start && cursor <= candidate.end,
  ) ?? { text: '', start: cursor, end: cursor };
  const before = token.text.slice(0, cursor - token.start);
  const separator = before.indexOf(querySelectorSeparator);
  if (separator >= 0) {
    return {
      kind: 'value',
      selector: before.slice(0, separator),
      partial: before.slice(separator + 1),
      replaceStart: token.start + separator + 1,
      replaceEnd: token.end,
    };
  }
  const parts = before.split(relationshipPathSeparator);
  const partial = parts.pop() ?? '';
  const afterCursor = token.text.slice(cursor - token.start);
  const selectorRest = afterCursor.search(/[.:]/);
  return {
    kind: 'field',
    path: parts,
    partial,
    replaceStart: cursor - partial.length,
    // Replace the rest of the part being edited, keeping a trailing `:value`.
    replaceEnd:
      selectorRest < 0
        ? token.end
        : cursor + selectorRest + (afterCursor[selectorRest] === ':' ? 1 : 0),
  };
};
