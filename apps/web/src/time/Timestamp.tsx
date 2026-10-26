import { Box, Tooltip } from '@mui/material';
import type { ReactNode } from 'react';
import {
  formatUtcInstant,
  type InstantStyle,
  type InstantValue,
  parseInstant,
} from './instantFormat';
import { useInstantFormat } from './useInstantFormat';

type TimestampProps = {
  value: InstantValue | null | undefined;
  style?: InstantStyle;
  /** Rendered when there is no value, e.g. “Never”. */
  fallback?: ReactNode;
  /**
   * Set to `false` inside links and buttons so the timestamp does not add a
   * nested tab stop; the tooltip remains available on hover.
   */
  focusable?: boolean;
  /** Follows the instant with its distance from now, e.g. `(3 days ago)`. */
  relative?: boolean;
};

/**
 * Renders an instant in the user's zone as a semantic `<time>` element with
 * the exact UTC value in a tooltip on hover and keyboard focus.
 */
export const Timestamp = ({
  value,
  style,
  fallback,
  focusable = true,
  relative = false,
}: TimestampProps) => {
  const { format, formatRelative } = useInstantFormat();
  if (value === null || value === undefined || value === '')
    return <>{fallback}</>;
  const date = parseInstant(value);
  if (!date) return <>{String(value)}</>;
  return (
    <Tooltip describeChild title={formatUtcInstant(date)}>
      <Box
        component="time"
        dateTime={date.toISOString()}
        // Focusable so keyboard users can reach the UTC tooltip.
        tabIndex={focusable ? 0 : undefined}
        sx={{
          '&:focus-visible': {
            outline: '2px solid',
            outlineColor: 'primary.main',
            outlineOffset: 2,
          },
        }}
      >
        {relative ? (
          // Each part wraps as a whole, so a narrow cell breaks between them.
          <>
            <Box component="span" sx={{ whiteSpace: 'nowrap' }}>
              {format(date, style)}
            </Box>{' '}
            <Box
              component="span"
              sx={{ color: 'text.secondary', whiteSpace: 'nowrap' }}
            >
              ({formatRelative(date)})
            </Box>
          </>
        ) : (
          format(date, style)
        )}
      </Box>
    </Tooltip>
  );
};
