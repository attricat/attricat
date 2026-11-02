import { IconButton, Popover, Stack, Tooltip, Typography } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { monoFontFamily } from '../../../app/theme';
import { compactIconSize } from '../../../components/iconSizes';
import { RecordIdIcon } from '../../../components/systemIcons';
import { CopyIconButton } from '../../../components/CopyIconButton';

export const RecordIdPopover = ({ recordId }: { recordId: string }) => {
  const { t } = useTranslation();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const label = t('records.viewRecordIdWithId', { recordId });
  return (
    <>
      <Tooltip title={t('records.viewRecordId')}>
        <IconButton
          aria-label={label}
          onClick={(event) => setAnchor(event.currentTarget)}
          size="small"
        >
          <RecordIdIcon size={compactIconSize} />
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
          <CopyIconButton label={t('records.copyRecordId')} value={recordId} />
        </Stack>
      </Popover>
    </>
  );
};
