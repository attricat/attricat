import InfoOutlinedIcon from '@mui/icons-material/InfoOutlined';
import { MenuItem, TextField, Tooltip } from '@mui/material';
import type { Attribute } from '../api';
import type { FileMetadata } from '../../files/schemas';
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
          label={attribute.code}
          onChange={(event) => onChange(event.target.value)}
          value={value}
        >
          <MenuItem value="">Not set</MenuItem>
          <MenuItem value="true">True</MenuItem>
          <MenuItem value="false">False</MenuItem>
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
        label={attribute.code}
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

const MigrationBadge = ({ message }: { message?: string }) => (
  <Tooltip
    title={
      message ??
      'Review is necessary for this field to migrate to the current schema version.'
    }
  >
    <InfoOutlinedIcon color="info" fontSize="small" />
  </Tooltip>
);
