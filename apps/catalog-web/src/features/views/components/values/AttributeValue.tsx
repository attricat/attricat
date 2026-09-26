import { Chip, Stack, Typography } from '@mui/material';
import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import type { Attribute } from '../../../entities/api';
import { fileDownloadUrl } from '../../../files/api';
import { FileThumbnail } from '../../../files/FileThumbnail';
import { formatAttributeValue } from './formatAttributeValue';

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
}: {
  attribute: Attribute;
  value: unknown;
  compact?: boolean;
}) => {
  const { t } = useTranslation();
  if (attribute.value_type === 'relationship' && isRelationshipValue(value)) {
    const items = value.items ?? [];
    if (compact)
      return (
        <Typography variant="body2">
          {items.length
            ? t('views.linked', { count: items.length })
            : t('views.notSet')}
        </Typography>
      );
    return (
      <Stack direction="row" spacing={1} sx={{ flexWrap: 'wrap' }}>
        {items.map((item) => (
          <Link
            key={item.id}
            params={{ entityId: item.id }}
            to="/entities/$entityId"
          >
            <Chip label={item.display ?? item.id} clickable />
          </Link>
        ))}
        {value.truncated && (
          <Chip label={t('views.moreLinkedEntities')} variant="outlined" />
        )}
        {!items.length && (
          <Typography color="text.secondary">{t('views.notSet')}</Typography>
        )}
      </Stack>
    );
  }
  if (attribute.value_type === 'file') {
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

    return (
      <Stack spacing={1}>
        {value.map((file) => (
          <Stack direction="row" key={file.id} spacing={1}>
            {attribute.file_policy?.image_only && (
              <FileThumbnail file={file} size={64} />
            )}
            <Typography
              component="a"
              href={fileDownloadUrl(file.id)}
              variant="body2"
            >
              {file.filename}
            </Typography>
          </Stack>
        ))}
      </Stack>
    );
  }

  if (
    attribute.value_type === 'boolean' &&
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
      {formatAttributeValue(attribute, value)}
    </Typography>
  );
};
