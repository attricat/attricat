import { Box, Stack, Typography, useTheme } from '@mui/material';
import { InfoIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type {
  Attribute,
  ComponentReference,
  StatusTransitionAccess,
} from '../api';
import type { FileMetadata } from '../../files/schemas';
import { attributeValueTypes } from '../valueTypes';
import { FileAttributeEditor } from '../../files/FileAttributeEditor';
import { RelationshipField } from './RelationshipField';
import { smallIconSize } from '../../../components/iconSizes';
import { ScalarAttributeEditor } from './ScalarAttributeEditor';
import type { MigrationFieldReview } from '../migrationReview';

/** Form editor for one attribute in the current context. */
export const RecordAttributeEditor = ({
  attribute,
  component,
  statusBaseline = null,
  inheritedStatus = null,
  statusTransitions,
  contextId,
  disabled,
  required,
  recordId,
  files,
  error,
  helperText,
  migrationReview,
  onChange,
  onRecordUpdated,
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
  recordId?: string;
  files: FileMetadata[];
  error?: string;
  helperText?: string;
  /** Shown below the editor when a migration asks to review the field. */
  migrationReview?: MigrationFieldReview;
  onChange: (value: string) => void;
  /** Receives the record version after a file change saved by this editor. */
  onRecordUpdated?: (updatedAt: string) => void;
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
        // Linked records can be opened once this record exists.
        linkToRecords={Boolean(recordId)}
        onChange={onChange}
        required={required}
        value={value}
      />
    ) : attribute.value_type === attributeValueTypes.file ? (
      <FileAttributeEditor
        attribute={attribute}
        contextId={contextId}
        disabled={effectiveDisabled}
        recordId={recordId}
        files={files}
        error={error}
        helperText={helperText}
        onRecordUpdated={onRecordUpdated}
        required={required}
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
  return migrationReview ? (
    // `display: contents` keeps the editor and its note laid out as siblings.
    <Box data-migration-review sx={{ display: 'contents' }}>
      {editor}
      <MigrationReviewNote review={migrationReview} />
    </Box>
  ) : (
    editor
  );
};

/** Why a migration asks to review the field, and the value it had before. */
const MigrationReviewNote = ({ review }: { review: MigrationFieldReview }) => {
  const { t } = useTranslation();
  const { palette } = useTheme();
  return (
    <Stack direction="row" spacing={1} sx={{ alignItems: 'flex-start' }}>
      <Box sx={{ display: 'flex', flexShrink: 0, pt: 0.25 }}>
        <InfoIcon aria-hidden color={palette.info.main} size={smallIconSize} />
      </Box>
      <Box>
        <Typography variant="body2">{review.message}</Typography>
        {review.previousValue && (
          <Typography
            color="text.secondary"
            sx={{ overflowWrap: 'anywhere' }}
            variant="body2"
          >
            {t('records.previousValue', { value: review.previousValue })}
          </Typography>
        )}
      </Box>
    </Stack>
  );
};
