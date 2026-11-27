import { EmptyValue } from '../../../../components/EmptyValue';
import { JsonInline } from '../../../../components/JsonValue';
import { Timestamp } from '../../../../time/Timestamp';
import type { Attribute } from '../../../records/api';
import { attributeValueTypes } from '../../../records/valueTypes';
import { principalConfiguration } from '../../../principals/principal';
import { PrincipalText } from '../../../principals/PrincipalValue';
import { formatAttributeValue } from './formatAttributeValue';

/**
 * An attribute value as inline text; datetime values render as timestamps,
 * user-or-team assignments as names, JSON values as one line of code and
 * missing values as `EmptyValue`.
 */
export const AttributeValueText = ({
  attribute,
  value,
}: {
  attribute: Attribute;
  value: unknown;
}) =>
  value === null || value === undefined ? (
    <EmptyValue />
  ) : attribute.value_type === attributeValueTypes.datetime &&
    typeof value === 'string' ? (
    <Timestamp value={value} />
  ) : principalConfiguration(attribute) ? (
    <PrincipalText value={value} />
  ) : attribute.value_type === attributeValueTypes.json ? (
    <JsonInline value={value} />
  ) : (
    <>{formatAttributeValue(attribute, value)}</>
  );
