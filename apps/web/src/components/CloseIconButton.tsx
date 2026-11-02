import type { IconButtonProps } from '@mui/material';
import { XIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from './iconSizes';
import { LabeledIconButton } from './LabeledIconButton';

/** Closes a panel, dialog or selection; `label` says what, where it helps. */
export const CloseIconButton = ({
  label,
  ...props
}: Omit<IconButtonProps, 'aria-label' | 'children' | 'disabled' | 'size'> & {
  label?: string;
}) => {
  const { t } = useTranslation();
  return (
    <LabeledIconButton
      {...props}
      label={label ?? t('common.close')}
      size="small"
    >
      <XIcon size={compactIconSize} />
    </LabeledIconButton>
  );
};
