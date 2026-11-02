import { IconButton, Tooltip, type IconButtonProps } from '@mui/material';
import { XIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from './iconSizes';

/** Closes a panel, dialog or selection; `label` says what, where it helps. */
export const CloseIconButton = ({
  label,
  ...props
}: Omit<IconButtonProps, 'aria-label' | 'children' | 'size'> & {
  label?: string;
}) => {
  const { t } = useTranslation();
  const name = label ?? t('common.close');
  return (
    <Tooltip title={name}>
      <IconButton {...props} aria-label={name} size="small">
        <XIcon size={compactIconSize} />
      </IconButton>
    </Tooltip>
  );
};
