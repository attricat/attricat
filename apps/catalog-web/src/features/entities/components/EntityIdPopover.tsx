import ContentCopyOutlinedIcon from '@mui/icons-material/ContentCopyOutlined';
import TagOutlinedIcon from '@mui/icons-material/TagOutlined';
import { IconButton, Popover, Stack, Tooltip, Typography } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { monoFontFamily } from '../../../app/theme';
import { copyToClipboard } from '../../../components/clipboard';
import { useToast } from '../../../components/useToast';

export const EntityIdPopover = ({
  alignWithText = false,
  entityId,
}: {
  alignWithText?: boolean;
  entityId: string;
}) => {
  const { t } = useTranslation();
  const { show } = useToast();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const copyEntityId = async () => {
    try {
      await copyToClipboard(entityId);
      show({ message: t('common.copied'), severity: 'success' });
    } catch {
      show({ message: t('common.copyFailed'), severity: 'error' });
    }
  };
  const label = t('entities.viewEntityIdWithId', { entityId });
  return (
    <>
      <Tooltip title={t('entities.viewEntityId')}>
        <IconButton
          aria-label={label}
          onClick={(event) => setAnchor(event.currentTarget)}
          size="small"
          sx={alignWithText ? { transform: 'translateY(-2px)' } : undefined}
        >
          <TagOutlinedIcon fontSize="inherit" />
        </IconButton>
      </Tooltip>
      <Popover
        anchorEl={anchor}
        anchorOrigin={{ horizontal: 'left', vertical: 'bottom' }}
        onClose={() => setAnchor(null)}
        open={Boolean(anchor)}
      >
        <Stack direction="row" spacing={1} sx={{ alignItems: 'center', p: 1 }}>
          <Typography component="code" sx={{ fontFamily: monoFontFamily }}>
            {entityId}
          </Typography>
          <Tooltip title={t('entities.copyEntityId')}>
            <IconButton
              aria-label={t('entities.copyEntityId')}
              onClick={() => void copyEntityId()}
              size="small"
            >
              <ContentCopyOutlinedIcon fontSize="inherit" />
            </IconButton>
          </Tooltip>
        </Stack>
      </Popover>
    </>
  );
};
