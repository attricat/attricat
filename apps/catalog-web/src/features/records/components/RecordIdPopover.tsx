import { IconButton, Popover, Stack, Tooltip, Typography } from '@mui/material';
import { CopyIcon, HashIcon } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { monoFontFamily } from '../../../app/theme';
import { copyToClipboard } from '../../../components/clipboard';
import { useToast } from '../../../components/useToast';
import { compactIconSize } from '../../../components/iconSizes';

export const RecordIdPopover = ({ recordId }: { recordId: string }) => {
  const { t } = useTranslation();
  const { show } = useToast();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const copyRecordId = async () => {
    try {
      await copyToClipboard(recordId);
      show({ message: t('common.copied'), severity: 'success' });
    } catch {
      show({ message: t('common.copyFailed'), severity: 'error' });
    }
  };
  const label = t('records.viewRecordIdWithId', { recordId });
  return (
    <>
      <Tooltip title={t('records.viewRecordId')}>
        <IconButton
          aria-label={label}
          onClick={(event) => setAnchor(event.currentTarget)}
          size="small"
        >
          <HashIcon size={compactIconSize} />
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
            {recordId}
          </Typography>
          <Tooltip title={t('records.copyRecordId')}>
            <IconButton
              aria-label={t('records.copyRecordId')}
              onClick={() => void copyRecordId()}
              size="small"
            >
              <CopyIcon size={compactIconSize} />
            </IconButton>
          </Tooltip>
        </Stack>
      </Popover>
    </>
  );
};
