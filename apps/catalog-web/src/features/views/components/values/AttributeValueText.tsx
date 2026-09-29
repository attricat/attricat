import { Timestamp } from '../../../../time/Timestamp';
import type { Attribute } from '../../../entities/api';
import { attributeValueTypes } from '../../../entities/valueTypes';
import { formatAttributeValue } from './formatAttributeValue';

/** An attribute value as inline text; datetime values render as timestamps. */
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
  ) : (
    <>{formatAttributeValue(attribute, value)}</>
  );
