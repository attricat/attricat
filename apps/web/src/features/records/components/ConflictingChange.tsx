import { Trans, useTranslation } from 'react-i18next';
import { AttributeValueText } from '../../views/components/values/AttributeValueText';
import type { Attribute } from '../api';
import { scalarValueForField } from '../attributeValues';
import { attributeLabel } from '../recordDisplay';
import { attributeValueTypes } from '../valueTypes';

const RawValue = ({ text }: { text: string }) => <>{text}</>;

/** A form field's value as the record shows it, such as a choice's label. */
const ConflictValue = ({
  attribute,
  fieldValue,
}: {
  attribute: Attribute;
  fieldValue: string;
}) => {
  const { t } = useTranslation();
  if (!fieldValue.trim()) return <>{t('records.empty')}</>;
  const scalar = scalarValueForField(attribute, fieldValue);
  return (
    <Trans
      components={{
        value: scalar ? (
          <AttributeValueText attribute={attribute} value={scalar.value} />
        ) : (
          // A value the field cannot parse shows as typed. Trans replaces a
          // component's children, so the text comes through a prop.
          <RawValue text={fieldValue} />
        ),
      }}
      i18nKey="records.quotedValue"
      t={t}
    />
  );
};

/** One field of an edit conflict: the user's change and the latest value. */
export const ConflictingChange = ({
  attribute,
  mine,
  theirs,
}: {
  attribute: Attribute;
  mine: string;
  theirs: string;
}) => {
  const { t } = useTranslation();
  // Relationship and file values are IDs; name the field.
  if (
    attribute.value_type === attributeValueTypes.relationship ||
    attribute.value_type === attributeValueTypes.file
  )
    return <>{attributeLabel(attribute)}</>;
  return (
    <Trans
      components={{
        mine: <ConflictValue attribute={attribute} fieldValue={mine} />,
        theirs: <ConflictValue attribute={attribute} fieldValue={theirs} />,
      }}
      i18nKey="records.conflictingChange"
      t={t}
      values={{ field: attributeLabel(attribute) }}
    />
  );
};
