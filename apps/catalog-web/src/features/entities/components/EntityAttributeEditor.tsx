import InfoOutlinedIcon from '@mui/icons-material/InfoOutlined';
import { MenuItem, TextField, Tooltip } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { Attribute } from '../api';
import type { FileMetadata } from '../../files/schemas';
import { attributeLabel } from '../entity-display';
import { attributeValueTypes } from '../value-types';
import { FileAttributeEditor } from '../../files/FileAttributeEditor';
import { RelationshipField } from './RelationshipField';

export const EntityAttributeEditor = ({
  attribute,
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
  const migrationBadge = showMigrationBadge ? (
    <MigrationBadge message={migrationReviewMessage} />
  ) : null;
  if (attribute.value_type === attributeValueTypes.relationship)
    return (
      <>
        {migrationBadge}
        <RelationshipField
          attribute={attribute}
          disabled={disabled}
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
          disabled={disabled}
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
          disabled={disabled}
          error={Boolean(error)}
          helperText={error ?? helperText}
          select
          label={attributeLabel(attribute)}
          onChange={(event) => onChange(event.target.value)}
          value={value}
        >
          <MenuItem value="">{t('entities.notSet')}</MenuItem>
          <MenuItem value="true">{t('entities.true')}</MenuItem>
          <MenuItem value="false">{t('entities.false')}</MenuItem>
        </TextField>
      </>
    );
  return (
    <>
      {migrationBadge}
      <TextField
        fullWidth
        disabled={disabled}
        error={Boolean(error)}
        helperText={error ?? helperText}
        label={attributeLabel(attribute)}
        onChange={(event) => onChange(event.target.value)}
        placeholder={
          attribute.value_type === attributeValueTypes.time
            ? '09:30:00 America/New_York'
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
  return (
    <Tooltip title={message ?? t('entities.migrationReview')}>
      <InfoOutlinedIcon color="info" fontSize="small" />
    </Tooltip>
  );
};
