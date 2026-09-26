import { Box, Paper, Stack } from '@mui/material';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { BrandIcon } from '../../components/BrandIcon';

export const authFormWidth = 360;

export const AuthFormShell = ({
  children,
  header,
  onSubmit,
}: {
  children: ReactNode;
  header?: ReactNode;
  onSubmit: () => void;
}) => {
  const { t } = useTranslation();
  return (
    <Stack
      sx={{
        alignItems: 'center',
        justifyContent: 'center',
        minHeight: '100dvh',
      }}
    >
      <Box aria-label={t('app.attricat')} sx={{ mb: 6 }}>
        <BrandIcon variant="wordmark" />
      </Box>
      {header}
      <Paper
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          onSubmit();
        }}
        sx={{ p: 4, width: authFormWidth }}
      >
        <Stack spacing={2}>{children}</Stack>
      </Paper>
    </Stack>
  );
};
