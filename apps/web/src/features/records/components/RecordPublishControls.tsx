import { GlobeIcon, GlobeOffIcon } from 'lucide-react';
import { Box, Chip, IconButton, Tooltip } from '@mui/material';
import { Trans, useTranslation } from 'react-i18next';
import { navigationRoutes } from '../../../components/navigation';
import { RouterLink } from '../../../components/RouterLink';
import { emptyValuePlaceholder } from '../../../components/placeholders';
import { publicationStatuses } from '../constants';
import { publicationReadinessText } from '../checkViolations';
import { compactIconSize } from '../../../components/iconSizes';
import { useInstantFormat } from '../../../time/useInstantFormat';
import type { RecordPublications } from './useRecordPublications';

/**
 * The selected context's publication status, as one icon whose tooltip also
 * explains publishing to callers who may publish, and its readiness. The Actions menu publishes or
 * withdraws the record.
 */
export const RecordPublishControls = ({
  canPublish,
  publications: { publication, readiness },
}: {
  canPublish: boolean;
  publications: RecordPublications;
}) => {
  const { t } = useTranslation();
  // A tooltip cannot host another tooltip, so the UTC value is inline.
  const { formatWithUtc } = useInstantFormat();
  const notReady = readiness?.ready === false;
  const readinessText = readiness && publicationReadinessText(readiness);
  const published = publication?.status === publicationStatuses.published;
  const statusLabel = publication
    ? t(`records.publication.${publication.status}`)
    : t('records.publication.channelDisabled');
  const statusDescription = !publication ? (
    <Trans
      components={{
        exportsLink: (
          <RouterLink color="inherit" to={navigationRoutes.exports} />
        ),
      }}
      i18nKey="records.publication.channelDisabledDescription"
      t={t}
    />
  ) : published && publication.published_at ? (
    t('records.publication.publishedDetails', {
      publishedAt: formatWithUtc(publication.published_at),
      publishedBy: publication.published_by_user_id ?? emptyValuePlaceholder,
    })
  ) : published ? null : (
    t('records.publication.notPublishedDescription')
  );
  return (
    <>
      <Tooltip
        describeChild
        enterTouchDelay={0}
        title={
          <>
            <Box component="strong" sx={{ display: 'block' }}>
              {statusLabel}
            </Box>
            {statusDescription && (
              <Box component="span" sx={{ display: 'block' }}>
                {statusDescription}
              </Box>
            )}
            {canPublish && (
              <Box component="span" sx={{ display: 'block', mt: 1 }}>
                {t('records.publication.actionsDescription')}
              </Box>
            )}
          </>
        }
      >
        <IconButton
          aria-label={statusLabel}
          size="small"
          sx={{
            color: !publication
              ? 'text.secondary'
              : published
                ? 'success.main'
                : 'warning.main',
          }}
        >
          {published ? (
            <GlobeIcon size={compactIconSize} />
          ) : (
            <GlobeOffIcon size={compactIconSize} />
          )}
        </IconButton>
      </Tooltip>
      {publication && notReady && (
        // Focusable so keyboard users can open the reasons; they also form
        // the chip's accessible description.
        <Tooltip describeChild enterTouchDelay={0} title={readinessText}>
          <Chip
            color="warning"
            label={t('records.publication.notReady')}
            size="small"
            tabIndex={0}
          />
        </Tooltip>
      )}
    </>
  );
};
