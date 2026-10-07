import { Chip, type ChipProps } from '@mui/material';
import { forwardRef } from 'react';

/** A chip rendered as a link element, for use with router links. */
export const AnchorChip = forwardRef<HTMLAnchorElement, ChipProps<'a'>>(
  (props, ref) => <Chip component="a" ref={ref} {...props} />,
);
