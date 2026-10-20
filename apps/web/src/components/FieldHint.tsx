import { Box } from '@mui/material';
import { InfoIcon } from 'lucide-react';
import type { ReactNode } from 'react';
import { captionIconSize } from './iconSizes';

/**
 * Helper text that says what a field holds and accepts, led by an info icon so
 * it reads apart from the field's errors. Pass it as a field's `helperText`
 * only when there is no error to show.
 */
export const FieldHint = ({ children }: { children: ReactNode }) => (
  <Box component="span" sx={{ display: 'flex', gap: 0.5 }}>
    {/* One line tall, so the icon stays beside the first line of long text. */}
    <Box
      component="span"
      sx={{
        alignItems: 'center',
        display: 'flex',
        flexShrink: 0,
        height: '1lh',
      }}
    >
      <InfoIcon aria-hidden size={captionIconSize} />
    </Box>
    <span>{children}</span>
  </Box>
);
