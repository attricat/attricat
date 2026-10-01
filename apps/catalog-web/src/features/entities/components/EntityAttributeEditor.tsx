import { MenuItem, TextField, Tooltip, useTheme } from '@mui/material';
import { InfoIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { Attribute, ComponentReference } from '../api';
import { EmailInput } from '../../views/components/EmailInput';
import { isEmailEditor } from '../../views/email';
import type { FileMetadata } from '../../files/schemas';
import { attributeLabel } from '../entityDisplay';
import { attributeValueTypes } from '../valueTypes';
import {
  booleanFieldValues,
  JSON_EDITOR_MIN_ROWS,
  JSON_VALUE_PLACEHOLDER,
  TIME_VALUE_PLACEHOLDER,
} from '../constants';
import { FileAttributeEditor } from '../../files/FileAttributeEditor';
import { RelationshipField } from './RelationshipField';
import { smallIconSize } from '../../../components/iconSizes';

export const EntityAttributeEditor = ({
  attribute,
  component,
  required,
  contextId,
  disabled,
  entityId,
  files,
  error,
  helperText,
  migrationReviewMessage,
  onChange,
  showMigrationBadge,
  value,
}: {
  attribute: Attribute;
  component?: ComponentReference | null;
  required?: boolean;
  contextId: string | null;
  disabled: boolean;
  entityId?: string;
  files: FileMetadata[];
  error?: string;
  helperText?: string;
  migrationReviewMessage?: string;
  onChange: (value: string) => void;
  showMigrationBadge: boolean;
  value: string;
}) => {
  const { t } = useTranslation();
  const providerUnavailable = attribute.extension_type?.available === false;
  const effectiveDisabled = disabled || providerUnavailable;
  const migrationBadge = showMigrationBadge ? (
    <MigrationBadge message={migrationReviewMessage} />
  ) : null;
  if (
    attribute.value_type === attributeValueTypes.string &&
    isEmailEditor(component)
  )
    return (
      <>
        {migrationBadge}
        <EmailInput
          label={attributeLabel(attribute)}
          value={value}
          onChange={onChange}
          disabled={effectiveDisabled}
          required={required}
          error={error}
          helperText={helperText}
        />
      </>
    );
  if (attribute.value_type === attributeValueTypes.relationship)
    return (
      <>
        {migrationBadge}
        <RelationshipField
          attribute={attribute}
          disabled={effectiveDisabled}
          error={error}
          helperText={helperText}
          onChange={onChange}
          value={value}
        />
      </>
    );
  if (attribute.value_type === attributeValueTypes.file)
    return (
      <>
        {migrationBadge}
        <FileAttributeEditor
          attribute={attribute}
          contextId={contextId}
          disabled={effectiveDisabled}
          entityId={entityId}
          files={files}
        />
      </>
    );
  if (attribute.value_type === attributeValueTypes.boolean)
    return (
      <>
        {migrationBadge}
        <TextField
          fullWidth
          disabled={effectiveDisabled}
          error={Boolean(error)}
          helperText={error ?? helperText}
          select
          label={attributeLabel(attribute)}
          onChange={(event) => onChange(event.target.value)}
          value={value}
        >
          <MenuItem value="">{t('entities.notSet')}</MenuItem>
          <MenuItem value={booleanFieldValues.true}>
            {t('entities.true')}
          </MenuItem>
          <MenuItem value={booleanFieldValues.false}>
            {t('entities.false')}
          </MenuItem>
        </TextField>
      </>
    );
  return (
    <>
      {migrationBadge}
      <TextField
        fullWidth
        disabled={effectiveDisabled}
        error={Boolean(error)}
        helperText={error ?? helperText}
        label={attributeLabel(attribute)}
        onChange={(event) => onChange(event.target.value)}
        multiline={attribute.value_type === attributeValueTypes.json}
        minRows={
          attribute.value_type === attributeValueTypes.json
            ? JSON_EDITOR_MIN_ROWS
            : undefined
        }
        placeholder={
          attribute.value_type === attributeValueTypes.time
            ? TIME_VALUE_PLACEHOLDER
            : attribute.value_type === attributeValueTypes.json
              ? JSON_VALUE_PLACEHOLDER
              : undefined
        }
        slotProps={{
          htmlInput: {
            inputMode:
              attribute.value_type === attributeValueTypes.number ||
              attribute.value_type === attributeValueTypes.integer
                ? 'decimal'
                : undefined,
          },
        }}
        type={
          attribute.value_type === attributeValueTypes.date
            ? 'date'
            : attribute.value_type === attributeValueTypes.number ||
                attribute.value_type === attributeValueTypes.integer
              ? 'number'
              : undefined
        }
        value={value}
      />
    </>
  );
};

const MigrationBadge = ({ message }: { message?: string }) => {
  const { t } = useTranslation();
  const { palette } = useTheme();
  return (
    <Tooltip title={message ?? t('entities.migrationReview')}>
      <InfoIcon color={palette.info.main} size={smallIconSize} />
    </Tooltip>
  );
};
