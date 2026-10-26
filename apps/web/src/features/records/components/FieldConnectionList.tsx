import { Stack, Typography } from '@mui/material';
import { Fragment } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import type { Attribute } from '../api';
import {
  attributeConnectionKinds,
  type AttributeConnection,
} from '../attributeConnections';
import { FieldRevealLink } from '../../views/components/FieldRevealLink';
import { attributeLabel } from '../recordDisplay';

/** Fields named before the rest are summed up as a count. */
const listedFieldLimit = 5;

type FieldProps = {
  attributes: readonly Attribute[];
  onReveal?: (code: string) => void;
};

/** A field's label, which leads to the field where fields can be revealed. */
const FieldName = ({
  attributes,
  code,
  onReveal,
}: FieldProps & { code: string }) => {
  const attribute = attributes.find((item) => item.code === code);
  const label = attribute ? attributeLabel(attribute) : code;
  return onReveal ? (
    <FieldRevealLink onClick={() => onReveal(code)}>{label}</FieldRevealLink>
  ) : (
    <>{label}</>
  );
};

/** Field labels in the language's list form, the first few as names. */
const FieldNames = ({
  attributes,
  codes,
  onReveal,
}: FieldProps & { codes: readonly string[] }) => {
  const { t, i18n } = useTranslation();
  const listed = codes.slice(0, listedFieldLimit);
  const more = codes.length - listed.length;
  // Placeholders stand for the listed fields, so each part can be a link.
  const parts = new Intl.ListFormat(i18n.language).formatToParts([
    ...listed.map((_, index) => String(index)),
    ...(more > 0
      ? [t('records.fieldConnections.moreFields', { count: more })]
      : []),
  ]);
  let element = 0;
  return (
    <>
      {parts.map((part, index) => {
        if (part.type === 'literal')
          return <Fragment key={index}>{part.value}</Fragment>;
        const code = listed[element++];
        return code === undefined ? (
          <Fragment key={index}>{part.value}</Fragment>
        ) : (
          <FieldName
            attributes={attributes}
            code={code}
            key={index}
            onReveal={onReveal}
          />
        );
      })}
    </>
  );
};

/** The sentence describing a connection, seen from one of its fields. */
const connectionKey = (
  connection: AttributeConnection,
  outgoing: boolean,
  alone: boolean,
) => {
  switch (connection.kind) {
    case attributeConnectionKinds.required:
      return 'records.fieldConnections.required';
    case attributeConnectionKinds.schemaCondition:
      return outgoing
        ? 'records.fieldConnections.schemaConditionSource'
        : 'records.fieldConnections.schemaConditionTarget';
    case attributeConnectionKinds.check:
      return alone
        ? 'records.fieldConnections.check'
        : 'records.fieldConnections.checkWith';
    case attributeConnectionKinds.transitionCondition:
      return outgoing
        ? 'records.fieldConnections.transitionConditionSource'
        : 'records.fieldConnections.transitionConditionTarget';
    case attributeConnectionKinds.statusLock:
      return outgoing
        ? 'records.fieldConnections.statusLockSource'
        : 'records.fieldConnections.statusLockTarget';
    case attributeConnectionKinds.uniqueKey:
      return alone
        ? 'records.fieldConnections.uniqueKey'
        : 'records.fieldConnections.uniqueKeyWith';
    case attributeConnectionKinds.rule:
      return alone
        ? 'records.fieldConnections.rule'
        : 'records.fieldConnections.ruleWith';
    case attributeConnectionKinds.dropdownLabel:
      return alone
        ? 'records.fieldConnections.dropdownLabel'
        : 'records.fieldConnections.dropdownLabelWith';
    case attributeConnectionKinds.include:
      return 'records.fieldConnections.include';
  }
};

/**
 * What ties one field to others, one sentence per connection. With
 * `onReveal`, the other fields it names lead to them.
 */
export const FieldConnectionList = ({
  attributeCode,
  attributes,
  connections,
  onReveal,
}: {
  attributeCode: string;
  /** Every attribute of the record, to name the connected fields. */
  attributes: readonly Attribute[];
  connections: readonly AttributeConnection[];
  onReveal?: (code: string) => void;
}) => {
  const { t } = useTranslation();
  return (
    <Stack component="ul" spacing={0.5} sx={{ m: 0, pl: 2 }}>
      {connections.map((connection, index) => {
        const others = connection.attributeCodes.filter(
          (code) => code !== attributeCode,
        );
        const outgoing = connection.sources?.includes(attributeCode) ?? false;
        const fields =
          outgoing || !connection.sources ? others : connection.sources;
        return (
          <Typography
            component="li"
            key={`${connection.kind}:${connection.name ?? ''}:${index}`}
            variant="body2"
          >
            <Trans
              components={{
                fields: (
                  <FieldNames
                    attributes={attributes}
                    codes={fields}
                    onReveal={onReveal}
                  />
                ),
                // A status connection starts from the status field.
                status: (
                  <FieldName
                    attributes={attributes}
                    code={connection.sources?.[0] ?? ''}
                    onReveal={onReveal}
                  />
                ),
              }}
              i18nKey={connectionKey(connection, outgoing, others.length === 0)}
              t={t}
              values={{ detail: connection.detail, name: connection.name }}
            />
          </Typography>
        );
      })}
    </Stack>
  );
};
