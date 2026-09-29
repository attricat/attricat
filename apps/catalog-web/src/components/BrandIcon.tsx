import { Box, useTheme } from '@mui/material';
import markLight from '../../design/assets/logos/mark.svg?url';
import markDark from '../../design/assets/logos/mark-dark.svg?url';
import wordmarkLight from '../../design/assets/logos/wordmark-light.svg?url';
import wordmarkDark from '../../design/assets/logos/wordmark-dark.svg?url';

const brandIconHeight = 32;
const brandIconWidths = { mark: 32, wordmark: 161 } as const;

export const BrandIcon = ({
  variant = 'mark',
}: {
  variant?: 'mark' | 'wordmark';
}) => {
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
