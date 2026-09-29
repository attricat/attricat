import { Button, MenuItem, TextField } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { Attribute } from '../entities/api';
import { attributeValueTypes } from '../entities/valueTypes';
import { sandboxBooleanValues, sandboxInputPlaceholders } from './constants';

const inputPlaceholder = (attribute: Attribute) => {
  if (attribute.value_type === attributeValueTypes.date)
    return sandboxInputPlaceholders.date;
  if (attribute.value_type === attributeValueTypes.datetime)
    return sandboxInputPlaceholders.datetime;
  if (attribute.value_type === attributeValueTypes.time)
    return sandboxInputPlaceholders.time;
  return undefined;
};

/** Unsaved input for one attribute in the blueprint view preview sandbox. */
export const SandboxAttributeEditor = ({
  attribute,
  onChange,
  value,
}: {
  attribute: Attribute;
  onChange: (value: string) => void;
  value: string;
}) => {
  const { t } = useTranslation();

  if (attribute.value_type === attributeValueTypes.relationship)
    return (
      <TextField
        disabled
        fullWidth
        helperText={t('blueprints.relationshipSandboxUnavailable')}
        label={attribute.code}
        value=""
      />
    );

  if (attribute.value_type === attributeValueTypes.boolean)
    return (
      <TextField
        fullWidth
        label={attribute.code}
        onChange={(event) => onChange(event.target.value)}
        select
        value={value}
      >
        <MenuItem value={sandboxBooleanValues.unset}>
          {t('blueprints.notSet')}
        </MenuItem>
        <MenuItem value={sandboxBooleanValues.true}>
          {t('blueprints.true')}
        </MenuItem>
        <MenuItem value={sandboxBooleanValues.false}>
          {t('blueprints.false')}
        </MenuItem>
      </TextField>
    );

  if (attribute.value_type === attributeValueTypes.file)
    return <Button>{t('blueprints.chooseOrDropFiles')}</Button>;

  return (
    <TextField
      fullWidth
      label={attribute.code}
      onChange={(event) => onChange(event.target.value)}
      placeholder={inputPlaceholder(attribute)}
      value={value}
    />
  );
};
