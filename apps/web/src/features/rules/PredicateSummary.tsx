import { Box, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { attributeLabel } from '../records/recordDisplay';
import { predicateTypes } from './constants';
import { optionLabel, type FindingAttribute } from './findingMessage';

/** A predicate as compiled: its type and whatever keys that type takes. */
type PredicateShape = { type: string } & Record<string, unknown>;

const SECONDS_PER_DAY = 86_400;

const isPredicate = (value: unknown): value is PredicateShape =>
  typeof value === 'object' &&
  value !== null &&
  typeof (value as { type?: unknown }).type === 'string';

const text = (value: unknown) =>
  typeof value === 'string' ? value : JSON.stringify(value);

/**
 * Says in words what a rule's predicate checks, nesting the parts of
 * `all_of`, `any_of`, `linked` and `referenced_by`. Attributes and their
 * fixed values are named by their labels where `attributes` knows them.
 */
export const PredicateSummary = ({
  predicate,
  attributes = [],
}: {
  predicate: PredicateShape;
  attributes?: readonly FindingAttribute[];
}) => {
  const { t } = useTranslation();
  const attribute = (code: unknown) => {
    const found = attributes.find((item) => item.code === code);
    return found ? attributeLabel(found) : text(code);
  };
  const value = (code: unknown, raw: unknown) => {
    const found = attributes.find((item) => item.code === code);
    const label =
      found && typeof raw === 'string' ? optionLabel(found, raw) : undefined;
    return `"${label ?? text(raw)}"`;
  };
  const op = (raw: unknown) =>
    t(`rules.preview.ops.${text(raw)}`, { defaultValue: text(raw) });
  const nested = (predicates: unknown) =>
    Array.isArray(predicates) && (
      <Box component="ul" sx={{ m: 0, pl: 3 }}>
        {predicates.filter(isPredicate).map((part, index) => (
          <li key={index}>
            <PredicateSummary attributes={attributes} predicate={part} />
          </li>
        ))}
      </Box>
    );
  const p = predicate;
  const line = (sentence: string) => (
    <Typography component="span" variant="body2">
      {sentence}
    </Typography>
  );
  switch (p.type) {
    case predicateTypes.required:
      return line(
        t('rules.preview.says.required', {
          attribute: attribute(p.attribute_code),
        }),
      );
    case predicateTypes.oneOf:
      return line(
        t('rules.preview.says.oneOf', {
          attribute: attribute(p.attribute_code),
          values: (Array.isArray(p.values) ? p.values : [])
            .map((item) => value(p.attribute_code, item))
            .join(', '),
        }),
      );
    case predicateTypes.compare:
      return line(
        t('rules.preview.says.compare', {
          attribute: attribute(p.attribute_code),
          op: op(p.op),
          other:
            p.other_attribute_code !== undefined
              ? attribute(p.other_attribute_code)
              : p.subject_attribute_code !== undefined
                ? t('rules.preview.says.subjectAttribute', {
                    attribute: text(p.subject_attribute_code),
                  })
                : value(p.attribute_code, p.value),
        }),
      );
    case predicateTypes.relativeDate: {
      const days = typeof p.offset_days === 'number' ? p.offset_days : 0;
      return line(
        t('rules.preview.says.relativeDate', {
          attribute: attribute(p.attribute_code),
          op: op(p.op),
          when:
            days === 0
              ? t('rules.preview.says.today')
              : days > 0
                ? t('rules.preview.says.daysFromNow', { count: days })
                : t('rules.preview.says.daysAgo', { count: -days }),
        }),
      );
    }
    case predicateTypes.stale:
      return line(
        t('rules.preview.says.stale', {
          attribute: attribute(p.attribute_code),
          count: Math.round(
            (typeof p.max_age_seconds === 'number' ? p.max_age_seconds : 0) /
              SECONDS_PER_DAY,
          ),
        }),
      );
    case predicateTypes.hasTag:
      return line(t('rules.preview.says.hasTag', { tag: text(p.tag) }));
    case predicateTypes.missingTag:
      return line(t('rules.preview.says.missingTag', { tag: text(p.tag) }));
    case predicateTypes.unique:
      return line(
        t('rules.preview.says.unique', {
          attributes: (Array.isArray(p.attribute_codes)
            ? p.attribute_codes
            : []
          )
            .map(attribute)
            .join(', '),
        }),
      );
    case predicateTypes.acyclic:
      return line(
        t('rules.preview.says.acyclic', {
          relationship: attribute(p.relationship_code),
        }),
      );
    case predicateTypes.linked:
      return (
        <>
          {line(
            t(`rules.preview.says.linked.${text(p.quantifier ?? 'all')}`, {
              relationship: attribute(p.relationship_code),
              defaultValue: t('rules.preview.says.linked.all', {
                relationship: attribute(p.relationship_code),
              }),
            }),
          )}
          {isPredicate(p.predicate) && nested([p.predicate])}
        </>
      );
    case predicateTypes.referencedBy:
      return (
        <>
          {line(
            t('rules.preview.says.referencedBy', {
              blueprint: text(p.blueprint_code),
              relationship: text(p.relationship_code),
              bounds:
                p.min !== undefined && p.max !== undefined
                  ? t('rules.preview.says.between', { min: p.min, max: p.max })
                  : p.min !== undefined
                    ? t('rules.preview.says.atLeast', { count: Number(p.min) })
                    : t('rules.preview.says.atMost', { count: Number(p.max) }),
            }),
          )}
          {isPredicate(p.predicate) && nested([p.predicate])}
        </>
      );
    case predicateTypes.allOf:
      return (
        <>
          {line(t('rules.preview.says.allOf'))}
          {nested(p.predicates)}
        </>
      );
    case predicateTypes.anyOf:
      return (
        <>
          {line(t('rules.preview.says.anyOf'))}
          {nested(p.predicates)}
        </>
      );
    default:
      // A predicate type added on the server before the app learns it keeps its code.
      return line(
        t(`rules.preview.predicateTypes.${p.type}`, { defaultValue: p.type }),
      );
  }
};
