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
  valueTypeKey,
  versionKey,
} from './constants';
import type { DefinitionReferences } from './definitionReferences';
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
  targetBlueprint?: string;
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
        targetBlueprint: at(targetBlueprintKey),
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
  targetBlueprint?: string;
  valueType?: string;
}) =>
  attribute.from
    ? i18n.t('definitionEditor.details.selectedAttribute', {
        from: attribute.from,
      })
    : attribute.targetBlueprint
      ? i18n.t('definitionEditor.details.relationshipTo', {
          target: attribute.targetBlueprint,
        })
      : (attribute.valueType ?? '');

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
      for (const segment of segments.slice(0, -1)) {
        const target = attributes.find(
          (attribute) => attribute.code === segment,
        )?.targetBlueprint;
        if (!target) return [];
        attributes = await references.blueprintAttributes(target);
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
          return {
            continues,
            detail: attributeDetail(attribute),
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
