import {
  Box,
  Collapse,
  Divider,
  IconButton,
  Skeleton,
  Stack,
  Typography,
} from '@mui/material';
import { ChevronDownIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { RecordView } from '../../views/components/RecordView';
import { VIEW_LAYOUT_SPACING } from '../../views/constants';
import { recordHeadingComponentId } from '../../views/components/blocks/RecordHeadingDefinition';
import { compactIconSize } from '../../../components/iconSizes';
import {
  RelatedRecordPreviewContext,
  type RelatedRecordPreviewState,
} from '../linkedRecord';
import { useResolvedRecordPreview } from '../useRecordContexts';
import { useRecordBlueprintRevision } from '../useRecordPreviewData';

const fieldSkeletons = ['first', 'second', 'third'];

const LinkedRecordValues = ({
  contextId,
  recordId,
}: {
  contextId: string;
  recordId: string;
}) => {
  const { t } = useTranslation();
  const resolved = useResolvedRecordPreview(recordId, contextId);
  const blueprint = useRecordBlueprintRevision(resolved.data?.record);
  if (resolved.isError || blueprint.isError)
    return (
      <Typography color="text.secondary" variant="body2">
        {t('records.relatedRecordUnavailable')}
      </Typography>
    );
  if (!resolved.data || !blueprint.data)
    return (
      // Holds the place of its fields, each a label above a value.
      <Stack
        aria-busy
        aria-label={t('records.loadingRelatedRecord')}
        spacing={VIEW_LAYOUT_SPACING}
      >
        {fieldSkeletons.map((key) => (
          <Stack key={key} spacing={0.5}>
            <Skeleton variant="text" width="30%" />
            <Skeleton variant="text" width="60%" />
          </Stack>
        ))}
      </Stack>
    );
  return (
    // Its own relationships show as links, so previews never nest.
    <RelatedRecordPreviewContext.Provider value={null}>
      <RecordView
        attributes={blueprint.data.attributes}
        contextId={contextId}
        fallbackVisibilityScope="detail"
        recordId={recordId}
        singleColumn
        skipComponentId={recordHeadingComponentId}
        values={resolved.data.values}
        view={blueprint.data.blueprint.views.detail}
      />
    </RelatedRecordPreviewContext.Provider>
  );
};

/** Opens or closes the linked record's details, at the end of its row. */
export const RelatedRecordPreviewToggle = ({
  label,
  preview,
}: {
  label: string;
  preview: RelatedRecordPreviewState;
}) => {
  const { t } = useTranslation();
  return (
    <IconButton
      aria-controls={preview.detailsId}
      aria-expanded={preview.expanded}
      aria-label={t('records.relatedRecordPreview', { record: label })}
      onClick={preview.toggle}
      size="small"
      sx={{ height: 24, ml: 'auto', width: 24 }}
    >
      <ChevronDownIcon
        size={compactIconSize}
        style={{
          transform: preview.expanded ? 'rotate(180deg)' : undefined,
          transition: 'transform 150ms',
        }}
      />
    </IconButton>
  );
};

/**
 * The saved values of the record a single relationship links to, read-only
 * and in the same context, below its pill within the field.
 */
export const RelatedRecordPreviewDetails = ({
  label,
  preview,
}: {
  label: string;
  preview: RelatedRecordPreviewState;
}) => {
  const { t } = useTranslation();
  return (
    <Collapse in={preview.expanded} unmountOnExit>
      <Box
        aria-label={t('records.relatedRecordPreview', { record: label })}
        id={preview.detailsId}
        role="region"
        // A disabled field fades its text; the linked record is still content.
        sx={{ color: 'text.primary', WebkitTextFillColor: 'currentcolor' }}
      >
        <Divider sx={{ my: 1.5 }} />
        <LinkedRecordValues
          contextId={preview.contextId}
          recordId={preview.recordId}
        />
      </Box>
    </Collapse>
  );
};
