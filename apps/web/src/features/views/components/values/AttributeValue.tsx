import { Box, Chip, Stack, Typography } from '@mui/material';
import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import type { ReactNode } from 'react';
import type { Attribute } from '../../../records/api';
import { fileDownloadUrl } from '../../../files/api';
import { ImageGallery } from '../../../files/ImageGallery';
import { NewTabIcon } from '../../../../components/NewTabIcon';
import { AttributeValueText } from './AttributeValueText';
import { formatAttributeValue } from './formatAttributeValue';
import { attributeValueTypes } from '../../../records/valueTypes';
import { statusConfiguration } from '../../../records/status';
import { StatusValue } from '../../controls/values';
import { principalConfiguration } from '../../../principals/principal';
import { PrincipalValue } from '../../../principals/PrincipalValue';
import {
  relatedRecordLinkProps,
  singleLinkedRecord,
  useRelatedRecordPreview,
} from '../../../records/linkedRecord';
import {
  RelatedRecordPreviewDetails,
  RelatedRecordPreviewToggle,
} from '../../../records/components/RelatedRecordPreview';

type RelationshipValue = {
  items?: { id: string; display?: string }[];
  truncated?: boolean;
};

const isRelationshipValue = (value: unknown): value is RelationshipValue =>
  typeof value === 'object' && value !== null && 'items' in value;

type FileValue = {
  id: string;
  filename: string;
  variants?: { kind: string }[];
}[];

const isFileValue = (value: unknown): value is FileValue =>
  Array.isArray(value) &&
  value.every(
    (item) =>
      typeof item === 'object' &&
      item !== null &&
      'id' in item &&
      'filename' in item &&
      typeof item.id === 'string' &&
      typeof item.filename === 'string',
  );

export const AttributeValue = ({
  attribute,
  value,
  compact = false,
  renderFilePanel,
  contextId,
  recordId,
}: {
  attribute: Attribute;
  value: unknown;
  compact?: boolean;
  contextId?: string;
  recordId?: string;
  renderFilePanel?: (fileId: string) => ReactNode;
}) => {
  const { t } = useTranslation();
  const linkedRecord = compact
    ? undefined
    : singleLinkedRecord(attribute, value);
  const preview = useRelatedRecordPreview(attribute, linkedRecord?.id);
  const status = statusConfiguration(attribute);
  if (status) return <StatusValue config={status} value={value} />;
  if (principalConfiguration(attribute))
    return <PrincipalValue value={value} />;
  if (
    attribute.value_type === attributeValueTypes.relationship &&
    isRelationshipValue(value)
  ) {
    const items = value.items ?? [];
    if (compact)
      return (
        <Typography variant="body2">
          {items.length
            ? t('views.linked', { count: items.length })
            : t('views.notSet')}
        </Typography>
      );
    const links = (
      <Stack
        direction="row"
        spacing={1}
        sx={{ alignItems: 'center', flexWrap: 'wrap' }}
        // A gap rather than margins lets the preview toggle align right.
        useFlexGap
      >
        {items.map((item) => (
          <Link
            key={item.id}
            params={{ recordId: item.id }}
            to="/records/$recordId"
            {...relatedRecordLinkProps}
          >
            <Chip
              clickable
              icon={<NewTabIcon />}
              label={item.display ?? item.id}
            />
          </Link>
        ))}
        {value.truncated && (
          <Chip label={t('views.moreLinkedRecords')} variant="outlined" />
        )}
        {!items.length && (
          <Typography color="text.secondary">{t('views.notSet')}</Typography>
        )}
        {preview && linkedRecord && (
          <RelatedRecordPreviewToggle
            label={linkedRecord.display ?? linkedRecord.id}
            preview={preview}
          />
        )}
      </Stack>
    );
    if (!preview || !linkedRecord) return links;
    // The link and the record it opens read as one field.
    return (
      <Box sx={{ border: 1, borderColor: 'divider', borderRadius: 1, p: 1.5 }}>
        {links}
        <RelatedRecordPreviewDetails
          label={linkedRecord.display ?? linkedRecord.id}
          preview={preview}
        />
      </Box>
    );
  }
  if (attribute.value_type === attributeValueTypes.file) {
    if (!isFileValue(value)) {
      return (
        <Typography variant="body2">{t('views.fileValueNotSet')}</Typography>
      );
    }

    if (compact) {
      return (
        <Typography variant="body2">
          {t('views.fileCount', { count: value.length })}
        </Typography>
      );
    }

    if (attribute.file_policy?.image_only) {
      return (
        <ImageGallery
          key={`${recordId ?? ''}:${contextId ?? ''}:${attribute.code}`}
          files={value}
          renderFilePanel={renderFilePanel}
        />
      );
    }

    return (
      <Stack spacing={1}>
        {value.map((file) => (
          <Stack key={file.id} spacing={1}>
            <Stack direction="row" spacing={1}>
              <Typography
                component="a"
                href={fileDownloadUrl(file.id)}
                variant="body2"
              >
                {file.filename}
              </Typography>
            </Stack>
            {renderFilePanel?.(file.id)}
          </Stack>
        ))}
      </Stack>
    );
  }

  if (
    attribute.value_type === attributeValueTypes.boolean &&
    value !== null &&
    value !== undefined &&
    !compact
  ) {
    return (
      <Chip
        color={value ? 'success' : 'default'}
        label={formatAttributeValue(attribute, value)}
        size="small"
      />
    );
  }
  return (
    <Typography
      color={
        value === null || value === undefined ? 'text.secondary' : undefined
      }
      variant="body2"
    >
      <AttributeValueText attribute={attribute} value={value} />
    </Typography>
  );
};
