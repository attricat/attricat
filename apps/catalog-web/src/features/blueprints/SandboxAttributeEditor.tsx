import { Button, TextField } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { Attribute, ComponentReference } from '../entities/api';
import { ScalarAttributeEditor } from '../entities/components/ScalarAttributeEditor';
import { attributeLabel } from '../entities/entityDisplay';
import { attributeValueTypes } from '../entities/valueTypes';

/**
 * Unsaved input for one attribute in the blueprint view preview sandbox. Scalar
 * attributes use the same editors as the entity form; relationships and files
 * need a saved entity, so the sandbox only shows where they would appear.
 */
export const SandboxAttributeEditor = ({
  attribute,
  component,
  onChange,
  value,
}: {
  attribute: Attribute;
  component?: ComponentReference | null;
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
        label={attributeLabel(attribute)}
        value=""
      />
    );
  if (attribute.value_type === attributeValueTypes.file)
    return <Button>{t('blueprints.chooseOrDropFiles')}</Button>;
  return (
    <ScalarAttributeEditor
      attribute={attribute}
      component={component}
      value={value}
      onChange={onChange}
      disabled={
        attribute.readonly === true ||
        attribute.extension_type?.available === false
      }
    />
  );
};
