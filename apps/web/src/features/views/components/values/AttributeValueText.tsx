import { JsonInline } from '../../../../components/JsonValue';
import { Timestamp } from '../../../../time/Timestamp';
import type { Attribute } from '../../../records/api';
import { attributeValueTypes } from '../../../records/valueTypes';
import { principalConfiguration } from '../../../principals/principal';
import { PrincipalText } from '../../../principals/PrincipalValue';
import { formatAttributeValue } from './formatAttributeValue';

/**
 * An attribute value as inline text; datetime values render as timestamps,
 * user-or-team assignments as names and JSON values as one line of code.
 */
export const AttributeValueText = ({
  attribute,
  value,
}: {
  attribute: Attribute;
  value: unknown;
}) =>
  attribute.value_type === attributeValueTypes.datetime &&
  typeof value === 'string' ? (
    <Timestamp value={value} />
  ) : principalConfiguration(attribute) ? (
    <PrincipalText value={value} />
  ) : attribute.value_type === attributeValueTypes.json &&
    value !== null &&
    value !== undefined ? (
    <JsonInline value={value} />
  ) : (
    <>{formatAttributeValue(attribute, value)}</>
  );
