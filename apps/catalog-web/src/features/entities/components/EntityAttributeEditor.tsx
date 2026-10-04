import { Tooltip, useTheme } from '@mui/material';
import { InfoIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { Attribute, ComponentReference } from '../api';
import type { StatusTransitionAccess } from '../recordControls';
import type { FileMetadata } from '../../files/schemas';
import { attributeValueTypes } from '../valueTypes';
import { FileAttributeEditor } from '../../files/FileAttributeEditor';
import { RelationshipField } from './RelationshipField';
import { smallIconSize } from '../../../components/iconSizes';
import { ScalarAttributeEditor } from './ScalarAttributeEditor';

/** Form editor for one attribute in the current context. */
export const EntityAttributeEditor = ({
  attribute,
  component,
  statusBaseline = null,
  inheritedStatus = null,
  statusTransitions,
  contextId,
  disabled,
  required,
  entityId,
  files,
  error,
  helperText,
  migrationReviewMessage,
  onChange,
  onEntityUpdated,
  showMigrationBadge,
  value,
}: {
  attribute: Attribute;
  component?: ComponentReference | null;
  statusBaseline?: string | null;
  inheritedStatus?: string | null;
  statusTransitions?: readonly StatusTransitionAccess[];
  contextId: string | null;
  disabled: boolean;
  required?: boolean;
  entityId?: string;
  files: FileMetadata[];
  error?: string;
  helperText?: string;
  migrationReviewMessage?: string;
  onChange: (value: string) => void;
  /** Receives the entity version after a file change saved by this editor. */
  onEntityUpdated?: (updatedAt: string) => void;
  showMigrationBadge: boolean;
  value: string;
}) => {
  const effectiveDisabled =
    disabled || attribute.extension_type?.available === false;
  const editor =
    attribute.value_type === attributeValueTypes.relationship ? (
      <RelationshipField
        attribute={attribute}
        disabled={effectiveDisabled}
        error={error}
        helperText={helperText}
        onChange={onChange}
        value={value}
      />
    ) : attribute.value_type === attributeValueTypes.file ? (
      <FileAttributeEditor
        attribute={attribute}
        contextId={contextId}
        disabled={effectiveDisabled}
        entityId={entityId}
        files={files}
        error={error}
        helperText={helperText}
        onEntityUpdated={onEntityUpdated}
      />
    ) : (
      <ScalarAttributeEditor
        attribute={attribute}
        component={component}
        statusBaseline={statusBaseline}
        inheritedStatus={inheritedStatus}
        statusTransitions={statusTransitions}
        value={value}
        disabled={effectiveDisabled}
        required={required}
        error={error}
        helperText={helperText}
        onChange={onChange}
      />
    );
  return (
    <>
      {showMigrationBadge && (
        <MigrationBadge message={migrationReviewMessage} />
      )}
      {editor}
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
