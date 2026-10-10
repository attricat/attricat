import { Timestamp } from '../../../../time/Timestamp';
import type { Attribute } from '../../../records/api';
import { attributeValueTypes } from '../../../records/valueTypes';
import { principalConfiguration } from '../../../principals/principal';
import { PrincipalText } from '../../../principals/PrincipalValue';
import { formatAttributeValue } from './formatAttributeValue';

/**
 * An attribute value as inline text; datetime values render as timestamps and
 * user-or-team assignments as names.
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
  ) : (
    <>{formatAttributeValue(attribute, value)}</>
  );
