import { Box, Stack, Typography } from '@mui/material';
import type { LucideIcon } from 'lucide-react';
import type { ReactNode } from 'react';
import { PageTitle, type PageTitleVariant } from './PageTitle';

// A titled header must name the icon of the system concept it presents; take
// it from `systemIcons` so pages match navigation.
type PageHeaderTitleProps =
  { icon: LucideIcon; title: ReactNode } | { icon?: never; title?: never };

export const PageHeader = ({
  actions,
  description,
  eyebrow,
  icon,
  title,
  titleVariant = 'h3',
}: PageHeaderTitleProps & {
  actions?: ReactNode;
  description?: ReactNode;
  eyebrow?: ReactNode;
  titleVariant?: PageTitleVariant;
}) => (
  <Stack
    direction={{ xs: 'column', sm: actions ? 'row' : 'column' }}
    spacing={1.5}
    sx={{ justifyContent: 'space-between' }}
  >
    <Box>
      {eyebrow && (
        <Typography color="primary" variant="overline">
          {eyebrow}
        </Typography>
      )}
      {icon && (
        <PageTitle icon={icon} variant={titleVariant}>
          {title}
        </PageTitle>
      )}
      {description && (
        <Typography color="text.secondary">{description}</Typography>
      )}
    </Box>
    {actions && <Box sx={{ alignSelf: { sm: 'center' } }}>{actions}</Box>}
  </Stack>
);
