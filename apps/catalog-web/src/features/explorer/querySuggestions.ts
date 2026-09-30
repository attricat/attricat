import type { Attribute } from '../entities/api';
import { attributeValueTypes } from '../entities/valueTypes';
import { relationshipPathSeparator } from './constants';
import {
  findAttribute,
  isSearchableLeaf,
  maximumQueryRelationshipHops,
  queryCursorContext,
  querySelectorSeparator,
  queryWildcard,
  resolveRelationshipPath,
  type QueryCursorContext,
  type QuerySchema,
} from './queryLanguage';

/** Keeps the dropdown scannable; typing narrows longer lists. */
export const maximumQuerySuggestions = 50;

const booleanValues = ['true', 'false'];

export type QuerySuggestion =
  | {
      kind: 'attribute';
      code: string;
      insert: string;
      attribute: Attribute;
    }
  | {
      kind: 'relationship';
      code: string;
      insert: string;
      attribute: Attribute;
      /** Display name of the linked blueprint, or its code. */
      target: string;
    }
  | {
      /** Searches every field of the related record: `relationship:term`. */
      kind: 'allFields';
      code: string;
      insert: string;
      target: string;
    }
  | { kind: 'global'; code: string; insert: string }
  | { kind: 'value'; code: string; insert: string };

export type QuerySuggestionResult = {
  context: QueryCursorContext;
  suggestions: QuerySuggestion[];
  /** Relationship targets are still loading for this context. */
  loading: boolean;
};

const rank = (code: string, partial: string) => {
  const candidate = code.toLowerCase();
  const typed = partial.toLowerCase();
  if (!typed) return 1;
  if (candidate.startsWith(typed)) return candidate === typed ? 0 : 1;
  return candidate.includes(typed) ? 2 : undefined;
};

const byRank = <T extends { code: string }>(items: T[], partial: string) =>
  items
    .map((item) => ({ item, rank: rank(item.code, partial) }))
    .filter(
      (entry): entry is { item: T; rank: number } => entry.rank !== undefined,
    )
    .sort((left, right) => left.rank - right.rank)
    .map((entry) => entry.item);

const targetName = (
  code: string,
  blueprintNames: Map<string, string>,
): string => blueprintNames.get(code) ?? code;

const fieldSuggestions = (
  schema: QuerySchema,
  context: Extract<QueryCursorContext, { kind: 'field' }>,
  blueprintNames: Map<string, string>,
): Omit<QuerySuggestionResult, 'context'> => {
  const depth = context.path.length;
  const resolved = resolveRelationshipPath(schema, context.path);
  if (resolved.status !== 'ok') {
    return { suggestions: [], loading: resolved.status === 'pending' };
  }
  const nested = depth > 0;
  const allowRelationships = depth < maximumQueryRelationshipHops;
  const prefix = context.path.map((part) => part + relationshipPathSeparator);
  const typedPath = prefix.join('');
  const suggestions: QuerySuggestion[] = [];
  if (depth === 1 && resolved.targetCode && !context.partial) {
    // `rel.` drills into a relationship; offer searching all of its fields
    // until the user starts typing a field name.
    suggestions.push({
      kind: 'allFields',
      code: context.path[0],
      insert: context.path[0] + querySelectorSeparator,
      target: targetName(resolved.targetCode, blueprintNames),
    });
  }
  if (!nested) {
    suggestions.push({
      kind: 'global',
      code: queryWildcard,
      insert: queryWildcard + querySelectorSeparator,
    });
  }
  for (const attribute of resolved.attributes) {
    if (attribute.value_type === attributeValueTypes.relationship) {
      if (!allowRelationships || !attribute.target_blueprint_code) continue;
      suggestions.push({
        kind: 'relationship',
        code: attribute.code,
        insert: attribute.code + relationshipPathSeparator,
        attribute,
        target: targetName(attribute.target_blueprint_code, blueprintNames),
      });
    } else if (
      attribute.value_type !== attributeValueTypes.file &&
      (!nested || isSearchableLeaf(attribute))
    ) {
      suggestions.push({
        kind: 'attribute',
        code: attribute.code,
        insert: attribute.code + querySelectorSeparator,
        attribute,
      });
    }
  }
  // The "all fields" row keeps its place: it matches the path, not `partial`.
  const [first, ...rest] = suggestions;
  const ranked =
    first?.kind === 'allFields'
      ? [first, ...byRank(rest, context.partial)]
      : byRank(suggestions, context.partial);
  return {
    suggestions: ranked
      .slice(0, maximumQuerySuggestions)
      .map((suggestion) =>
        suggestion.kind === 'allFields'
          ? suggestion
          : { ...suggestion, code: typedPath + suggestion.code },
      ),
    loading: false,
  };
};

const selectorAttribute = (schema: QuerySchema, selector: string) => {
  const parts = selector.split(relationshipPathSeparator);
  const path = resolveRelationshipPath(schema, parts.slice(0, -1));
  if (path.status !== 'ok') return undefined;
  return findAttribute(path.attributes, parts[parts.length - 1], () => true);
};

const valueSuggestions = (
  schema: QuerySchema,
  context: Extract<QueryCursorContext, { kind: 'value' }>,
): QuerySuggestion[] => {
  const attribute = selectorAttribute(schema, context.selector);
  if (attribute?.value_type !== attributeValueTypes.boolean) return [];
  return byRank(
    booleanValues.map((value) => ({
      kind: 'value' as const,
      code: value,
      insert: `${value} `,
    })),
    context.partial,
  ).filter((suggestion) => suggestion.code !== context.partial);
};

export const querySuggestions = (
  schema: QuerySchema,
  query: string,
  cursor: number,
  blueprintNames: Map<string, string>,
): QuerySuggestionResult => {
  const context = queryCursorContext(query, cursor);
  if (context.kind === 'value') {
    return {
      context,
      suggestions: valueSuggestions(schema, context),
      loading: false,
    };
  }
  return { context, ...fieldSuggestions(schema, context, blueprintNames) };
};

/** Inserts a suggestion, returning the new query and caret position. */
export const applyQuerySuggestion = (
  query: string,
  context: QueryCursorContext,
  suggestion: QuerySuggestion,
) => {
  const start =
    suggestion.kind === 'allFields'
      ? // Replace `rel.partial` with `rel:`.
        context.replaceStart - suggestion.code.length - 1
      : context.replaceStart;
  let end = context.replaceEnd;
  const last = suggestion.insert.at(-1);
  // Do not duplicate a separator that already follows the replaced text.
  if (
    (last === relationshipPathSeparator || last === querySelectorSeparator) &&
    query[end] === last
  ) {
    end += 1;
  }
  const next = query.slice(0, start) + suggestion.insert + query.slice(end);
  return { query: next, cursor: start + suggestion.insert.length };
};

/** Relationship target blueprints the schema needs to resolve `query`. */
export const queryTargetCodes = (
  schema: QuerySchema,
  query: string,
): string[] => {
  const codes = new Set<string>();
  const add = (path: string[]) => {
    let attributes: Attribute[] | undefined = schema.attributes;
    for (const name of path) {
      if (!attributes) return;
      const relationship = findAttribute(
        attributes,
        name,
        (attribute) =>
          attribute.value_type === attributeValueTypes.relationship,
      );
      const code = relationship?.target_blueprint_code;
      if (!code) return;
      codes.add(code);
      attributes = schema.targets.get(code);
    }
  };
  for (const term of query.split(/\s+/)) {
    const separator = term.indexOf(querySelectorSeparator);
    const selector = separator < 0 ? term : term.slice(0, separator);
    // The last part is a leaf (or still being typed), never a hop to load.
    add(selector.split(relationshipPathSeparator).slice(0, -1));
  }
  return [...codes];
};
