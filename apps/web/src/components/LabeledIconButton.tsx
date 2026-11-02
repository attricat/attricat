import { IconButton, Tooltip, type IconButtonProps } from '@mui/material';

/**
 * An icon button named by its tooltip, so sighted and screen reader users get
 * the same words. A tooltip cannot follow a disabled button; wrap one that can
 * be disabled by hand.
 */
export const LabeledIconButton = ({
  label,
  ...props
}: Omit<IconButtonProps, 'aria-label' | 'disabled'> & { label: string }) => (
  <Tooltip title={label}>
    <IconButton {...props} aria-label={label} />
  </Tooltip>
);
