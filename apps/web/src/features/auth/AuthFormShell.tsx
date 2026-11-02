import { Box, Paper, Stack, Typography } from '@mui/material';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { BrandIcon } from '../../components/BrandIcon';
import { useInitialFocus } from '../../components/initialFocus';

export const authFormWidth = 360;

/** Names the page; the first thing inside an auth form. */
export const AuthFormTitle = ({ children }: { children: ReactNode }) => (
  <Typography component="h1" variant="h5">
    {children}
  </Typography>
);

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
  const formRef = useInitialFocus<HTMLFormElement>();
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
        ref={formRef}
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          onSubmit();
        }}
        sx={{ p: 4, width: authFormWidth }}
      >
        <Stack spacing={2} useFlexGap>
          {children}
        </Stack>
      </Paper>
    </Stack>
  );
};
