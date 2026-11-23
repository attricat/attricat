import {
  Box,
  Button,
  Collapse,
  Divider,
  Skeleton,
  Stack,
  Typography,
  useTheme,
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
import { fieldSkeletons } from '../constants';
import { compactOutlinedActionButtonSx } from '../../../components/CompactOutlinedActionButton';

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
  const { transitions } = useTheme();
  // A worded button beside the pill: an icon alone at the field's edge read
  // as the field's own dropdown arrow and was easy to miss.
  return (
    <Button
      aria-controls={preview.detailsId}
      aria-expanded={preview.expanded}
      aria-label={t(
        preview.expanded
          ? 'records.hideRelatedRecordPreview'
          : 'records.relatedRecordPreview',
        { record: label },
      )}
      color="primary"
      endIcon={
        <ChevronDownIcon
          size={compactIconSize}
          style={{
            transform: preview.expanded ? 'rotate(180deg)' : undefined,
            transition: transitions.create('transform', {
              duration: transitions.duration.shortest,
            }),
          }}
        />
      }
      onClick={preview.toggle}
      size="small"
      sx={{
        ...compactOutlinedActionButtonSx,
        '& .MuiButton-endIcon': { ml: 0.5 },
      }}
    >
      {t(
        preview.expanded
          ? 'records.hideRecordDetails'
          : 'records.showRecordDetails',
      )}
    </Button>
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
