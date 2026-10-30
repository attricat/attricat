import i18n from '../../i18n';
import type {
  TextRange,
  YamlEntry,
  YamlOutline,
} from '../../components/yamlOutline';
import {
  aliasKey,
  attributePathSeparator,
  attributesKey,
  codeKey,
  definitionKinds,
  includesKey,
  referencePages,
  schemaExtensions,
  type DefinitionKind,
} from './constants';
import {
  referenceCandidates,
  type Candidate,
} from './definitionReferenceCandidates';
import { embeddedReferences } from './definitionEmbeddedReferences';
import type { DefinitionReferences } from './definitionReferences';
import {
  definitionReferenceKinds,
  definitionSchemas,
  schemaAtPath,
  type DefinitionReferenceKind,
} from './definitionSchema';

/** A string value in the document that names something else. */
export type ReferenceValue = {
  entry: YamlEntry;
  range: TextRange;
  reference: DefinitionReferenceKind;
  value: string;
};

type ReferenceContext = {
  outline: YamlOutline;
  references: DefinitionReferences;
};

/**
 * References whose lookups list everything that exists, so a value missing
 * from them is an error. Attribute lookups omit what mixins contribute.
 */
const exhaustiveReferences = new Set<DefinitionReferenceKind>([
  definitionReferenceKinds.blueprint,
  definitionReferenceKinds.context,
  definitionReferenceKinds.mixin,
  definitionReferenceKinds.role,
]);

/** References that name a value declared elsewhere in the same document. */
const documentReferences = new Set<DefinitionReferenceKind>([
  definitionReferenceKinds.attribute,
  definitionReferenceKinds.attributePath,
  definitionReferenceKinds.includeAttribute,
  definitionReferenceKinds.relationshipAttribute,
  definitionReferenceKinds.relationshipPath,
]);

const contains = (range: TextRange, offset: number) =>
  offset >= range.start && offset <= range.end;

export const referenceValues = (
  kind: DefinitionKind,
  outline: YamlOutline,
): ReferenceValue[] => {
  const root = definitionSchemas[kind];
  return outline.entries.flatMap((entry) => {
    const { path, value, valueRange } = entry;
    if (typeof value !== 'string' || !valueRange) return [];
    const node = schemaAtPath(root, path, outline.valueAt)?.node;
    // Array fields carry their hints on the array, not on each element.
    const array =
      typeof path.at(-1) === 'number'
        ? schemaAtPath(root, path.slice(0, -1), outline.valueAt)?.node
        : undefined;
    const reference =
      node?.[schemaExtensions.reference] ?? array?.[schemaExtensions.reference];
    return reference
      ? [{ entry, range: valueRange, reference, value }]
      : kind === definitionKinds.blueprint
        ? embeddedReferences(outline.text, entry).map((embedded) => ({
            ...embedded,
            entry,
          }))
        : [];
  });
};

export const referenceAt = (
  kind: DefinitionKind,
  outline: YamlOutline,
  offset: number,
) =>
  referenceValues(kind, outline).find(({ range }) => contains(range, offset));

/**
 * The lookup candidate a reference names. `known` is false when the lookup
 * returned nothing, for example without permission to list it.
 */
export const resolveReference = async (
  context: ReferenceContext,
  { entry, reference, value }: ReferenceValue,
): Promise<{ found?: Candidate; known: boolean }> => {
  const candidates = await referenceCandidates(
    context,
    reference,
    entry.path,
    value,
  );
  const found = candidates.find(
    (candidate) =>
      String(candidate.value) === value ||
      String(candidate.value) === `${value}${attributePathSeparator}`,
  );
  return { found, known: candidates.length > 0 };
};

/** Hover text for a reference value: what it names, or that it is missing. */
export const referenceHover = async (
  context: ReferenceContext,
  target: ReferenceValue,
): Promise<string[] | undefined> => {
  const { found, known } = await resolveReference(context, target);
  if (found) {
    const title = `**${found.label ?? String(found.value)}**`;
    return [
      found.detail ? `${title} — ${found.detail}` : title,
      found.documentation,
    ].filter((content): content is string => Boolean(content));
  }
  return known && exhaustiveReferences.has(target.reference)
    ? [
        i18n.t('definitionEditor.hover.unknownReference', {
          value: target.value,
        }),
      ]
    : undefined;
};

/** Exhaustive references that name nothing in the workspace. */
export const unknownReferences = async (
  kind: DefinitionKind,
  context: ReferenceContext,
) => {
  const targets = referenceValues(kind, context.outline).filter(
    ({ reference, value }) => value && exhaustiveReferences.has(reference),
  );
  const resolved = await Promise.all(
    targets.map(async (target) => ({
      ...(await resolveReference(context, target)),
      target,
    })),
  );
  return resolved
    .filter(({ found, known }) => known && !found)
    .map(({ target }) => ({
      message: i18n.t('definitionEditor.diagnostics.unknownReference', {
        value: target.value,
      }),
      range: target.range,
    }));
};

const declaration = (
  outline: YamlOutline,
  table: string,
  key: string,
  value: string,
) =>
  outline.entries.find(
    ({ path, value: declared }) =>
      path.length === 3 &&
      path[0] === table &&
      path[2] === key &&
      declared === value,
  )?.valueRange;

/** Where the document declares what a reference names, for Go to Definition. */
export const referenceDeclaration = (
  outline: YamlOutline,
  { reference, value }: ReferenceValue,
): TextRange | undefined => {
  if (!documentReferences.has(reference)) return undefined;
  const [first = ''] = value.split(attributePathSeparator);
  return reference === definitionReferenceKinds.includeAttribute
    ? declaration(outline, includesKey, aliasKey, first)
    : declaration(outline, attributesKey, codeKey, first);
};

/** A page that shows what an exhaustive reference names. */
export const referenceLink = async (
  context: ReferenceContext,
  { reference, value }: ReferenceValue,
) => {
  const pages = referencePages;
  switch (reference) {
    case definitionReferenceKinds.blueprint:
    case definitionReferenceKinds.mixin: {
      const blueprint = (await context.references.blueprints()).find(
        ({ code }) => code === value,
      );
      return (
        blueprint && {
          tooltip: i18n.t('definitionEditor.links.blueprint'),
          url: pages.blueprint(blueprint.id),
        }
      );
    }
    case definitionReferenceKinds.context:
      return (await context.references.contexts()).includes(value)
        ? {
            tooltip: i18n.t('definitionEditor.links.contexts'),
            url: pages.contexts,
          }
        : undefined;
    case definitionReferenceKinds.role:
      return (await context.references.roles()).includes(value)
        ? { tooltip: i18n.t('definitionEditor.links.roles'), url: pages.roles }
        : undefined;
    default:
      return undefined;
  }
};
