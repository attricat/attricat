import {
  ChevronDownIcon,
  GlobeIcon,
  InfoIcon,
  RefreshCwIcon,
  SendIcon,
  Undo2Icon,
} from 'lucide-react';
import {
  Button,
  Chip,
  IconButton,
  ListItemIcon,
  ListItemText,
  Menu,
  MenuItem,
  Tooltip,
} from '@mui/material';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { emptyValuePlaceholder } from '../../../components/placeholders';
import { publicationStatuses } from '../constants';
import { publicationReadinessText } from '../checkViolations';
import { compactIconSize } from '../../../components/iconSizes';
import { useInstantFormat } from '../../../time/useInstantFormat';
import type { RecordPublications } from './useRecordPublications';

/**
 * The selected context's publication status and readiness, and, for callers
 * who may publish, the menu that publishes or withdraws the record.
 */
export const RecordPublishControls = ({
  canPublish,
  publications: {
    isPending,
    notReadyChannels,
    publication,
    publish,
    publishAll,
    readiness,
    unpublish,
  },
}: {
  canPublish: boolean;
  publications: RecordPublications;
}) => {
  const { t } = useTranslation();
  // A tooltip cannot host another tooltip, so the UTC value is inline.
  const { formatWithUtc } = useInstantFormat();
  const menuId = useId();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const choose = (action: () => void) => () => {
    setAnchor(null);
    action();
  };
  const notReady = readiness?.ready === false;
  const readinessText = readiness && publicationReadinessText(readiness);
  return (
    <>
      {!publication && (
        <Tooltip title={t('records.publication.channelDisabledDescription')}>
          <Button color="inherit" size="small" variant="text">
            {t('records.publication.channelDisabled')}
          </Button>
        </Tooltip>
      )}
      {publication && (
        <Tooltip
          title={
            publication.status === publicationStatuses.published &&
            publication.published_at
              ? t('records.publication.publishedDetails', {
                  publishedAt: formatWithUtc(publication.published_at),
                  publishedBy:
                    publication.published_by_user_id ?? emptyValuePlaceholder,
                })
              : ''
          }
        >
          <Chip
            color={
              publication.status === publicationStatuses.published
                ? 'success'
                : 'default'
            }
            label={t(`records.publication.${publication.status}`)}
            size="small"
          />
        </Tooltip>
      )}
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
      {canPublish && (
        <>
          <Tooltip
            enterTouchDelay={0}
            title={t('records.publication.actionsDescription')}
          >
            <IconButton
              aria-label={t('records.publication.actionsHelp')}
              size="small"
            >
              <InfoIcon size={compactIconSize} />
            </IconButton>
          </Tooltip>
          <Button
            aria-controls={anchor ? menuId : undefined}
            aria-expanded={Boolean(anchor)}
            aria-haspopup="menu"
            color="inherit"
            disabled={isPending}
            endIcon={<ChevronDownIcon />}
            onClick={(event) => setAnchor(event.currentTarget)}
            size="small"
          >
            {t('records.publication.actions')}
          </Button>
          <Menu
            anchorEl={anchor}
            anchorOrigin={{ horizontal: 'right', vertical: 'bottom' }}
            id={menuId}
            onClose={() => setAnchor(null)}
            open={Boolean(anchor)}
            transformOrigin={{ horizontal: 'right', vertical: 'top' }}
          >
            {publication && (
              <MenuItem disabled={notReady} onClick={choose(publish)}>
                <ListItemIcon>
                  {publication.status === publicationStatuses.notPublished ? (
                    <SendIcon size={compactIconSize} />
                  ) : (
                    <RefreshCwIcon size={compactIconSize} />
                  )}
                </ListItemIcon>
                <ListItemText secondary={notReady ? readinessText : undefined}>
                  {publication.status === publicationStatuses.notPublished
                    ? t('records.publish')
                    : t('records.republish')}
                </ListItemText>
              </MenuItem>
            )}
            <MenuItem
              disabled={notReadyChannels.length > 0}
              onClick={choose(publishAll)}
            >
              <ListItemIcon>
                <GlobeIcon size={compactIconSize} />
              </ListItemIcon>
              <ListItemText
                secondary={
                  notReadyChannels.length > 0
                    ? t('records.publication.channelsNotReady', {
                        channels: notReadyChannels
                          .map((channel) => channel.context_code)
                          .join(', '),
                      })
                    : undefined
                }
              >
                {t('records.publishAllChannels')}
              </ListItemText>
            </MenuItem>
            {publication?.status === publicationStatuses.published && (
              <MenuItem
                onClick={choose(unpublish)}
                sx={{ color: 'warning.main' }}
              >
                <ListItemIcon sx={{ color: 'inherit' }}>
                  <Undo2Icon size={compactIconSize} />
                </ListItemIcon>
                <ListItemText>{t('records.unpublish')}</ListItemText>
              </MenuItem>
            )}
          </Menu>
        </>
      )}
    </>
  );
};
