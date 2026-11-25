import i18n from '../../i18n';
import {
  aliasKey,
  attributePathSeparator,
  attributesKey,
  codeKey,
  fromKey,
  includesKey,
  maxAttributePathHops,
  requiredSortPrefix,
  targetBlueprintKey,
  targetBlueprintsKey,
  valueTypeKey,
  versionKey,
} from './constants';
import type { DefinitionReferences } from './definitionReferences';
import {
  allowedTargetBlueprints,
  frontierAttributes,
  type PathFields,
} from '../records/relationshipTargets';
import { formatList } from '../../i18n/listFormat';
import {
  definitionReferenceKinds,
  type DefinitionReferenceKind,
} from './definitionSchema';
import type { YamlOutline, YamlPath } from '../../components/yamlOutline';
import { viewComponentPlacement, viewComponents } from './viewComponents';
import { attributeValueTypes } from '../records/valueTypes';
import { blueprintKinds } from '../blueprints/constants';

export type Candidate = {
  continues?: boolean;
  detail?: string;
  documentation?: string;
  label?: string;
  sortText?: string;
  value: string | number | boolean;
};

type DocumentAttribute = {
  code: string;
  from?: string;
  targetBlueprints?: string[];
  valueType?: string;
};

const documentAttributes = (outline: YamlOutline): DocumentAttribute[] =>
  outline.entries.flatMap(({ path, value }) => {
    if (
      path.length !== 3 ||
      path[0] !== attributesKey ||
      path[2] !== codeKey ||
      typeof value !== 'string'
    )
      return [];
    const at = (key: string) => {
      const found = outline.valueAt([attributesKey, path[1], key]);
      return typeof found === 'string' ? found : undefined;
    };
    return [
      {
        code: value,
        from: at(fromKey),
        targetBlueprints: allowedTargetBlueprints({
          target_blueprint_code: at(targetBlueprintKey),
          target_blueprint_codes: outline
            .valuesAt([attributesKey, path[1], targetBlueprintsKey])
            .filter((code) => typeof code === 'string'),
        }),
        valueType: at(valueTypeKey),
      },
    ];
  });

/** Attributes the document declares, then those of its scope. */
const knownAttributes = (
  outline: YamlOutline,
  references: DefinitionReferences,
): DocumentAttribute[] => [
  ...documentAttributes(outline),
  ...(references.scopeAttributes ?? []),
];

const attributeDetail = (attribute: {
  from?: string;
  targetBlueprints?: string[];
  valueType?: string;
}) =>
  attribute.from
    ? i18n.t('definitionEditor.details.selectedAttribute', {
        from: attribute.from,
      })
    : attribute.targetBlueprints?.length
      ? i18n.t('definitionEditor.details.relationshipTo', {
          target: formatList(attribute.targetBlueprints, 'disjunction'),
        })
      : (attribute.valueType ?? '');

/** Reads definition-reference attributes for `frontierAttributes`. */
const referencePathFields: PathFields<DocumentAttribute> = {
  code: (attribute) => attribute.code,
  valueType: (attribute) => attribute.valueType ?? '',
  targets: (attribute) => attribute.targetBlueprints ?? [],
};

/**
 * The attributes of the blueprints a path hop links to, merged as the API
 * resolves the next segment: codes they declare differently are left out,
 * and `partial` names the blueprints of codes only some of them declare.
 */
const pathTargetAttributes = async (
  targets: string[],
  references: DefinitionReferences,
) => {
  const lists = await Promise.all(
    targets.map((target) => references.blueprintAttributes(target)),
  );
  const frontier = frontierAttributes(
    targets.map((code, index) => ({ code, attributes: lists[index] })),
    referencePathFields,
  );
  return {
    attributes: frontier.attributes.filter(
      (attribute) => !frontier.conflicts.has(attribute.code),
    ),
    partial: frontier.partial,
  };
};

const includes = (outline: YamlOutline) =>
  outline.entries.flatMap(({ path, value }) => {
    if (
      path.length !== 3 ||
      path[0] !== includesKey ||
      path[2] !== aliasKey ||
      typeof value !== 'string'
    )
      return [];
    const code = outline.valueAt([includesKey, path[1], codeKey]);
    const version = outline.valueAt([includesKey, path[1], versionKey]);
    return [
      {
        alias: value,
        code: typeof code === 'string' ? code : undefined,
        version: typeof version === 'number' ? version : undefined,
      },
    ];
  });

const pathSegments = (prefix: string) => prefix.split(attributePathSeparator);

export const referenceCandidates = async (
  {
    outline,
    references,
  }: { outline: YamlOutline; references: DefinitionReferences },
  reference: DefinitionReferenceKind,
  path: YamlPath,
  prefix: string,
): Promise<Candidate[]> => {
  const sibling = (key: string) => outline.valueAt([...path.slice(0, -1), key]);
  switch (reference) {
    case definitionReferenceKinds.blueprint:
    case definitionReferenceKinds.mixin:
      return (await references.blueprints())
        .filter(
          (blueprint) =>
            reference === definitionReferenceKinds.blueprint ||
            blueprint.kind === blueprintKinds.mixin,
        )
        .map((blueprint) => ({
          detail: blueprint.name,
          documentation: i18n.t('definitionEditor.details.blueprintKind', {
            kind: blueprint.kind,
            version: blueprint.version,
          }),
          value: blueprint.code,
        }));
    case definitionReferenceKinds.mixinVersion: {
      const code = sibling(codeKey);
      const mixin = (await references.blueprints()).find(
        (blueprint) => blueprint.code === code,
      );
      return mixin
        ? [
            {
              detail: i18n.t('definitionEditor.details.latestVersion'),
              value: mixin.version,
            },
          ]
        : [];
    }
    case definitionReferenceKinds.attribute:
    case definitionReferenceKinds.relationshipAttribute:
      return knownAttributes(outline, references)
        .filter(
          (attribute) =>
            reference === definitionReferenceKinds.attribute ||
            attribute.valueType === attributeValueTypes.relationship,
        )
        .map((attribute) => ({
          detail: attributeDetail(attribute),
          value: attribute.code,
        }));
    case definitionReferenceKinds.attributePath:
    case definitionReferenceKinds.relationshipPath: {
      // A relationship path ends on a relationship rather than past one.
      const relationshipsOnly =
        reference === definitionReferenceKinds.relationshipPath;
      const maxHops = maxAttributePathHops - (relationshipsOnly ? 1 : 0);
      const segments = pathSegments(prefix);
      const hops = segments.length - 1;
      if (hops > maxHops) return [];
      let attributes: DocumentAttribute[] = knownAttributes(
        outline,
        references,
      );
      let partial = new Map<string, string[]>();
      // A hop may link to several blueprints; the path continues in each.
      for (const segment of segments.slice(0, -1)) {
        const targets = attributes.find(
          (attribute) => attribute.code === segment,
        )?.targetBlueprints;
        if (!targets?.length) return [];
        ({ attributes, partial } = await pathTargetAttributes(
          targets,
          references,
        ));
      }
      return attributes
        .filter(
          (attribute) =>
            !relationshipsOnly ||
            attribute.valueType === attributeValueTypes.relationship,
        )
        .map((attribute) => {
          const continues =
            attribute.valueType === attributeValueTypes.relationship &&
            hops < maxHops;
          const declaredOn = partial.get(attribute.code);
          return {
            continues,
            detail: declaredOn
              ? i18n.t('definitionEditor.details.onlyOn', {
                  detail: attributeDetail(attribute),
                  blueprints: formatList(declaredOn),
                })
              : attributeDetail(attribute),
            label: attribute.code,
            value: [
              ...segments.slice(0, -1),
              continues
                ? `${attribute.code}${attributePathSeparator}`
                : attribute.code,
            ].join(attributePathSeparator),
          };
        });
    }
    case definitionReferenceKinds.includeAttribute: {
      const [alias, ...rest] = pathSegments(prefix);
      if (!rest.length) {
        return includes(outline).map((include) => ({
          continues: true,
          detail: i18n.t('definitionEditor.details.include', {
            code: include.code,
            version: include.version,
          }),
          label: include.alias,
          value: `${include.alias}${attributePathSeparator}`,
        }));
      }
      const include = includes(outline).find(
        (candidate) => candidate.alias === alias,
      );
      if (!include?.code) return [];
      const code = sibling(codeKey);
      return (
        await references.blueprintAttributes(include.code, include.version)
      ).map((attribute) => ({
        detail: attributeDetail(attribute),
        label: attribute.code,
        sortText: attribute.code === code ? requiredSortPrefix : undefined,
        value: `${alias}${attributePathSeparator}${attribute.code}`,
      }));
    }
    case definitionReferenceKinds.role:
      return (await references.roles()).map((role) => ({
        detail: i18n.t('definitionEditor.details.role'),
        value: role,
      }));
    case definitionReferenceKinds.context:
      return (await references.contexts()).map((context) => ({
        detail: i18n.t('definitionEditor.details.context'),
        value: context,
      }));
    case definitionReferenceKinds.attributeEditor:
      return (
        await editorCandidates(outline, references, path.slice(0, -1))
      ).map((editor) => ({
        detail: i18n.t('definitionEditor.details.editor', {
          extension: editor.extensionName,
          version: editor.version,
        }),
        value: editor.id,
      }));
    case definitionReferenceKinds.viewComponent:
      return componentCandidates(outline, path.slice(0, -1)).map(
        (component) => ({
          detail: i18n.t('definitionEditor.details.component', {
            version: component.version,
          }),
          value: component.id,
        }),
      );
  }
};

/**
 * Attribute editors that fit the detail field holding `editorPath`: an
 * editor applies only to the value types it declares.
 */
export const editorCandidates = async (
  outline: YamlOutline,
  references: DefinitionReferences,
  editorPath: YamlPath,
) => {
  const field = outline.valueAt([...editorPath.slice(0, -1), 'field']);
  const valueType = documentAttributes(outline).find(
    (attribute) => attribute.code === field,
  )?.valueType;
  return ((await references.attributeEditors?.()) ?? []).filter(
    (editor) =>
      valueType === undefined || editor.valueTypes.includes(valueType),
  );
};

export const componentCandidates = (
  outline: YamlOutline,
  referencePath: YamlPath,
) => {
  const attributes = documentAttributes(outline);
  const placement = viewComponentPlacement(referencePath, (path) => {
    const value = outline.valueAt(path);
    return typeof value === 'string' ? value : undefined;
  });
  const valueType =
    placement.field === undefined
      ? undefined
      : attributes.find((attribute) => attribute.code === placement.field)
          ?.valueType;
  return viewComponents.filter(
    (component) =>
      (placement.placement === undefined ||
        component.placements.includes(placement.placement)) &&
      (placement.capability === undefined ||
        component.capabilities.includes(placement.capability)) &&
      (valueType === undefined ||
        !component.value_types.length ||
        component.value_types.includes(valueType)),
  );
};
