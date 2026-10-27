import { Box, useTheme } from '@mui/material';
import {
  brandIconHeight,
  brandIconWidths,
  type BrandVariant,
} from './brandIconSizes';
import markLight from '../../design/assets/logos/mark.svg?url';
import markDark from '../../design/assets/logos/mark-dark.svg?url';
import wordmarkLight from '../../design/assets/logos/wordmark-light.svg?url';
import wordmarkDark from '../../design/assets/logos/wordmark-dark.svg?url';

export const BrandIcon = ({ variant = 'mark' }: { variant?: BrandVariant }) => {
  const dark = useTheme().palette.mode === 'dark';
  const source =
    variant === 'mark'
      ? dark
        ? markDark
        : markLight
      : dark
        ? wordmarkDark
        : wordmarkLight;
  return (
    <Box
      alt=""
      component="img"
      src={source}
      sx={{
        display: 'block',
        height: brandIconHeight,
        width: brandIconWidths[variant],
      }}
    />
  );
};
