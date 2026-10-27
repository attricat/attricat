import { isJsonObject } from '../../api/jsonObject';
import { parse } from 'smol-toml';
import type { Rule } from '../rules/schemas';
import type { Attribute } from './api';
import { attributeLabel } from './recordDisplay';
import {
  statusConfiguration,
  statusCovers,
  statusCodeLabel,
  statusOptionLabel,
} from './status';
import {
  recordSchemaChecksKeyword,
  relationshipPredicateTypes,
} from './constants';

/** How a blueprint ties an attribute to others. */
export const attributeConnectionKinds = {
  required: 'required',
  schemaCondition: 'schema_condition',
  check: 'check',
  transitionCondition: 'transition_condition',
  statusLock: 'status_lock',
  uniqueKey: 'unique_key',
  rule: 'rule',
  dropdownLabel: 'dropdown_label',
  include: 'include',
} as const;
export type AttributeConnectionKind =
  (typeof attributeConnectionKinds)[keyof typeof attributeConnectionKinds];

export type AttributeConnection = {
  kind: AttributeConnectionKind;
  /** Every attribute the connection involves; each of them shows it. */
  attributeCodes: readonly string[];
  /**
   * The attributes the connection starts from, when it has a direction: a
   * status that locks others, or a schema condition's `if` side.
   */
  sources?: readonly string[];
  /** The check, key, rule, mixin or status attribute behind it. */
  name?: string;
  /** The mixin version an included attribute comes from. */
  version?: number;
  /** Status options, a transition, or the mixin attribute selected. */
  detail?: string;
};

type JsonObject = Record<string, unknown>;

const strings = (value: unknown) =>
  Array.isArray(value)
    ? value.filter((item): item is string => typeof item === 'string')
    : [];
const unique = (codes: readonly string[]) => [...new Set(codes)];

/**
 * Codes of this record's attributes that a predicate reads. Inside `linked`
 * and `referenced_by`, `attribute_code` names an attribute of the other
 * record and `subject_attribute_code` one of this record.
 */
export const predicateAttributeCodes = (
  predicate: unknown,
  linked = false,
): string[] => {
  if (!isJsonObject(predicate)) return [];
  const own = linked
    ? [predicate.subject_attribute_code]
    : [
        predicate.attribute_code,
        predicate.other_attribute_code,
        ...strings(predicate.attribute_codes),
      ];
  // `referenced_by` names a relationship of the referencing blueprint.
  const relationship =
    predicate.type === relationshipPredicateTypes.linked ||
    predicate.type === relationshipPredicateTypes.acyclic
      ? [predicate.relationship_code]
      : [];
  const nestedLinked =
    linked ||
    predicate.type === relationshipPredicateTypes.linked ||
    predicate.type === relationshipPredicateTypes.referencedBy;
  const nested = [
    predicate.predicate,
    ...(Array.isArray(predicate.predicates) ? predicate.predicates : []),
  ].flatMap((item) => predicateAttributeCodes(item, nestedLinked));
  return unique(
    [...own, ...relationship, ...nested].filter(
      (code): code is string => typeof code === 'string',
    ),
  );
};

/** Attributes a JSON Schema part constrains at the record's root. */
const schemaAttributeCodes = (schema: unknown) =>
  isJsonObject(schema)
    ? unique([
        ...(isJsonObject(schema.properties)
          ? Object.keys(schema.properties)
          : []),
        ...strings(schema.required),
      ])
    : [];

const schemaCondition = (
  sources: readonly string[],
  targets: readonly string[],
): AttributeConnection[] =>
  sources.length > 0 && targets.length > 0
    ? [
        {
          kind: attributeConnectionKinds.schemaCondition,
          attributeCodes: unique([...sources, ...targets]),
          sources,
        },
      ]
    : [];

/** `if`/`then`/`else` at the root and in its root compositions. */
const conditionalSchemas = (schema: JsonObject): AttributeConnection[] => [
  ...('if' in schema
    ? schemaCondition(
        schemaAttributeCodes(schema.if),
        unique([
          ...schemaAttributeCodes(schema.then),
          ...schemaAttributeCodes(schema.else),
        ]),
      )
    : []),
  ...['allOf', 'anyOf', 'oneOf'].flatMap((keyword) =>
    Array.isArray(schema[keyword])
      ? schema[keyword].filter(isJsonObject).flatMap(conditionalSchemas)
      : [],
  ),
];

const recordSchemaConnections = (
  recordSchema: unknown,
): AttributeConnection[] => {
  if (!isJsonObject(recordSchema)) return [];
  const dependentRequired = isJsonObject(recordSchema.dependentRequired)
    ? recordSchema.dependentRequired
    : {};
  const dependentSchemas = isJsonObject(recordSchema.dependentSchemas)
    ? recordSchema.dependentSchemas
    : {};
  const declaredChecks = recordSchema[recordSchemaChecksKeyword];
  const checks = Array.isArray(declaredChecks)
    ? declaredChecks.filter(isJsonObject)
    : [];
  return [
    ...strings(recordSchema.required).map((code) => ({
      kind: attributeConnectionKinds.required,
      attributeCodes: [code],
    })),
    ...Object.entries(dependentRequired).flatMap(([source, targets]) =>
      schemaCondition([source], strings(targets)),
    ),
    ...Object.entries(dependentSchemas).flatMap(([source, schema]) =>
      schemaCondition([source], schemaAttributeCodes(schema)),
    ),
    ...conditionalSchemas(recordSchema),
    ...checks.flatMap((check) => {
      const codes = predicateAttributeCodes(check.predicate);
      const name =
        typeof check.message === 'string' ? check.message : check.code;
      return codes.length > 0 && typeof name === 'string'
        ? [
            {
              kind: attributeConnectionKinds.check,
              attributeCodes: codes,
              name,
            },
          ]
        : [];
    }),
  ];
};

const statusConnections = (
  attributes: readonly Attribute[],
): AttributeConnection[] =>
  attributes.flatMap((status) => {
    const config = statusConfiguration(status);
    if (!config) return [];
    const name = attributeLabel(status);
    // One connection for the attributes locked in the same options.
    const lockingOptions = new Map<string, string[]>();
    for (const attribute of attributes) {
      const options = config.options
        .filter(
          (option) =>
            option.lock &&
            statusCovers(option.lock, attribute.code, status.code),
        )
        .map(statusOptionLabel);
      if (options.length === 0) continue;
      const detail = options.join(', ');
      lockingOptions.set(detail, [
        ...(lockingOptions.get(detail) ?? []),
        attribute.code,
      ]);
    }
    const locks = [...lockingOptions].map(([detail, codes]) => ({
      kind: attributeConnectionKinds.statusLock,
      attributeCodes: [status.code, ...codes],
      sources: [status.code],
      name,
      detail,
    }));
    const conditions = (config.transitions ?? []).flatMap((transition) => {
      const codes = unique(
        (transition.conditions ?? []).flatMap((condition) =>
          predicateAttributeCodes(condition.predicate),
        ),
      ).filter((code) => code !== status.code);
      if (codes.length === 0) return [];
      const from =
        transition.from === null
          ? undefined
          : statusCodeLabel(config, transition.from);
      const to =
        transition.to === null
          ? undefined
          : statusCodeLabel(config, transition.to);
      return [
        {
          kind: attributeConnectionKinds.transitionCondition,
          attributeCodes: [status.code, ...codes],
          sources: [status.code],
          name,
          detail: `${from ?? '—'} → ${to ?? '—'}`,
        },
      ];
    });
    return [...locks, ...conditions];
  });

const ruleConnections = (rules: readonly Rule[]): AttributeConnection[] =>
  rules.flatMap((rule) => {
    const plan = isJsonObject(rule.compiled_plan) ? rule.compiled_plan : {};
    const enforcement = isJsonObject(plan.enforcement) ? plan.enforcement : {};
    const transitions = Array.isArray(enforcement.transitions)
      ? enforcement.transitions.filter(isJsonObject)
      : [];
    const codes = unique([
      ...predicateAttributeCodes(plan.predicate),
      ...transitions.flatMap((transition) =>
        strings([transition.attribute_code]),
      ),
    ]);
    return codes.length > 0
      ? [
          {
            kind: attributeConnectionKinds.rule,
            attributeCodes: codes,
            name: rule.name,
          },
        ]
      : [];
  });

/** Parts that only the blueprint's definition source records. */
const definitionConnections = (definition: string): AttributeConnection[] => {
  let source: JsonObject;
  try {
    source = parse(definition);
  } catch {
    return [];
  }
  const includes = new Map(
    (Array.isArray(source.includes) ? source.includes : [])
      .filter(isJsonObject)
      .map((include) => [include.alias, include]),
  );
  const views = isJsonObject(source.views) ? source.views : {};
  const dropdown = isJsonObject(views.dropdown_option)
    ? strings(views.dropdown_option.fields)
    : [];
  return [
    ...(Array.isArray(source.unique_keys) ? source.unique_keys : [])
      .filter(isJsonObject)
      .flatMap((key) => {
        const codes = strings(key.attributes);
        return codes.length > 0 && typeof key.code === 'string'
          ? [
              {
                kind: attributeConnectionKinds.uniqueKey,
                attributeCodes: codes,
                name: key.code,
              },
            ]
          : [];
      }),
    ...(dropdown.length > 0
      ? [
          {
            kind: attributeConnectionKinds.dropdownLabel,
            attributeCodes: dropdown,
          },
        ]
      : []),
    ...(Array.isArray(source.attributes) ? source.attributes : [])
      .filter(isJsonObject)
      .flatMap((attribute) => {
        if (
          typeof attribute.code !== 'string' ||
          typeof attribute.from !== 'string'
        )
          return [];
        const include = includes.get(attribute.from.split('.')[0]);
        return [
          {
            kind: attributeConnectionKinds.include,
            attributeCodes: [attribute.code],
            ...(include && typeof include.code === 'string'
              ? {
                  name: include.code,
                  ...(typeof include.version === 'number' && {
                    version: include.version,
                  }),
                }
              : { name: attribute.from }),
            detail: attribute.from,
          },
        ];
      }),
  ];
};

/**
 * Everything in a blueprint revision, and in the rules enabled for it, that
 * ties attributes to each other or to a part of the blueprint.
 */
export const blueprintConnections = ({
  attributes,
  recordSchema,
  definition,
  rules = [],
}: {
  attributes: readonly Attribute[];
  recordSchema: unknown;
  definition: unknown;
  rules?: readonly Rule[];
}): AttributeConnection[] => [
  ...recordSchemaConnections(recordSchema),
  ...statusConnections(attributes),
  ...(typeof definition === 'string' ? definitionConnections(definition) : []),
  ...ruleConnections(rules),
];
