import { Box, Card, Stack, Typography } from '@mui/material';
import type { LucideIcon } from 'lucide-react';
import { type ReactNode, useId } from 'react';
import { smallIconSize } from '../../components/iconSizes';

type ProfileSectionProps = {
  children: ReactNode;
  description?: ReactNode;
  icon: LucideIcon;
  title: ReactNode;
};

export const ProfileSection = ({
  children,
  description,
  icon: Icon,
  title,
}: ProfileSectionProps) => {
  const headingId = useId();
  return (
    <Card aria-labelledby={headingId} component="section" sx={{ p: 6 }}>
      <Stack direction="row" spacing={3} sx={{ mb: 5 }}>
        <Box sx={{ color: 'text.secondary', display: 'flex', pt: 0.25 }}>
          <Icon aria-hidden size={smallIconSize} />
        </Box>
        <Box>
          <Typography component="h2" id={headingId} variant="h4">
            {title}
          </Typography>
          {description && (
            <Typography color="text.secondary" sx={{ mt: 1 }} variant="body2">
              {description}
            </Typography>
          )}
        </Box>
      </Stack>
      {children}
    </Card>
  );
};
