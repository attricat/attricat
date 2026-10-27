import { Box, Stack, Typography } from '@mui/material';
import { useId } from 'react';
import { useTranslation } from 'react-i18next';
import { EmptyState } from '../../../components/EmptyState';
import { JsonBlock } from '../../../components/JsonValue';
import { SystemTagIcon } from '../../../components/systemIcons';
import { visibleSystemTags } from '../systemTags';
import { SystemTagChips } from './SystemTagChips';

type Props = {
  tags: readonly string[];
  metadata: Record<string, unknown>;
};

/**
 * A record's system tags and metadata, read-only. Workflows, agents,
 * extensions and API clients write them; they are outside the blueprint.
 */
export const RecordSystemAnnotationsPanel = ({ tags, metadata }: Props) => {
  const { t } = useTranslation();
  const tagsHeadingId = useId();
  const metadataHeadingId = useId();
  const hasTags = visibleSystemTags(tags).length > 0;
  const hasMetadata = Object.keys(metadata).length > 0;
  if (!hasTags && !hasMetadata) {
    return (
      <EmptyState
        description={t('records.systemAnnotations.emptyDescription')}
        icon={SystemTagIcon}
        title={t('records.systemAnnotations.empty')}
      />
    );
  }
  return (
    <Stack spacing={3}>
      <Box component="section" aria-labelledby={tagsHeadingId}>
        <Typography id={tagsHeadingId} component="h2" variant="subtitle1">
          {t('records.systemAnnotations.tags')}
        </Typography>
        <Box sx={{ mt: 1 }}>
          {hasTags ? (
            <SystemTagChips tags={tags} />
          ) : (
            <Typography color="text.secondary" variant="body2">
              {t('records.systemAnnotations.noTags')}
            </Typography>
          )}
        </Box>
      </Box>
      <Box component="section" aria-labelledby={metadataHeadingId}>
        <Typography id={metadataHeadingId} component="h2" variant="subtitle1">
          {t('records.systemAnnotations.metadata')}
        </Typography>
        <Box sx={{ mt: 1 }}>
          {hasMetadata ? (
            <JsonBlock value={metadata} />
          ) : (
            <Typography color="text.secondary" variant="body2">
              {t('records.systemAnnotations.noMetadata')}
            </Typography>
          )}
        </Box>
      </Box>
    </Stack>
  );
};
