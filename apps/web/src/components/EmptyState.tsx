import { Box, Typography } from '@mui/material';
import type { LucideIcon } from 'lucide-react';
import type { ElementType, ReactNode } from 'react';

type Props = {
  description?: ReactNode;
  /** The icon of the system concept that is missing. */
  icon: LucideIcon;
  title: ReactNode;
  /** Render the title as a heading where it starts a page section. */
  titleComponent?: ElementType;
};

/**
 * What a list or section is missing: a quiet concept icon, a short title and,
 * optionally, why it matters. Render it inside the surface that would hold the
 * items, such as a Paper, a list, or a full-width table cell.
 */
export const EmptyState = ({
  description,
  icon: Icon,
  title,
  titleComponent = 'p',
}: Props) => (
  <Box sx={{ px: 4, py: 8, textAlign: 'center' }}>
    <Box sx={{ color: 'text.secondary' }}>
      <Icon aria-hidden />
    </Box>
    <Typography component={titleComponent} sx={{ mt: 2 }} variant="subtitle1">
      {title}
    </Typography>
    {description && (
      <Typography color="text.secondary" sx={{ mt: 1 }} variant="body2">
        {description}
      </Typography>
    )}
  </Box>
);
