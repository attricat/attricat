import { Button, Tooltip } from '@mui/material';
import { SparklesIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { SMART_FILL_BUTTON_OFFSET } from '../constants';

type Props = {
  disabled: boolean;
  onClick: () => void;
};

/** Floating action that opens the entity agent for smart fill. */
export const SmartFillButton = ({ disabled, onClick }: Props) => {
  const { t } = useTranslation();
  return (
    <Tooltip title={t('entities.smartFill')}>
      <span>
        <Button
          aria-label={t('entities.smartFill')}
          disabled={disabled}
          onClick={onClick}
          startIcon={<SparklesIcon />}
          sx={{
            bottom: SMART_FILL_BUTTON_OFFSET,
            position: 'fixed',
            right: SMART_FILL_BUTTON_OFFSET,
            zIndex: 1,
          }}
          variant="contained"
        >
          {t('entities.smartFill')}
        </Button>
      </span>
    </Tooltip>
  );
};
