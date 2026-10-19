import { ButtonBase, Tooltip, type SxProps, type Theme } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { copyToClipboard } from '../../../components/clipboard';
import { useToast } from '../../../components/useToast';

/**
 * A value shown as text that copies itself when clicked. It works in
 * read-only fields and table cells alike, and never opens what contains it.
 */
export const CopyableText = ({
  value,
  sx,
}: {
  value: string;
  sx?: SxProps<Theme>;
}) => {
  const { t } = useTranslation();
  const { show } = useToast();
  const copy = async () => {
    try {
      await copyToClipboard(value);
      show({ message: t('common.copied'), severity: 'success' });
    } catch {
      show({ message: t('common.copyFailed'), severity: 'error' });
    }
  };
  return (
    <Tooltip title={t('views.copyValue')}>
      <ButtonBase
        aria-label={t('views.copyValueNamed', { value })}
        onClick={(event) => {
          event.stopPropagation();
          void copy();
        }}
        sx={[
          {
            borderRadius: 1,
            mx: -0.5,
            overflowWrap: 'anywhere',
            px: 0.5,
            py: 0.25,
            textAlign: 'left',
            typography: 'body2',
            '&:hover': { bgcolor: 'action.hover' },
            '&.Mui-focusVisible': {
              outline: '2px solid',
              outlineColor: 'primary.main',
            },
          },
          ...(Array.isArray(sx) ? sx : [sx]),
        ]}
      >
        {value}
      </ButtonBase>
    </Tooltip>
  );
};
