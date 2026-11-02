import { Box, Paper, Typography } from '@mui/material';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Timestamp } from '../../time/Timestamp';
import type { ExtensionInstallation } from './managementApi';

const Detail = ({
  children,
  label,
}: {
  children: ReactNode;
  label: string;
}) => (
  <>
    <Typography color="text.secondary" component="dt" variant="subtitle2">
      {label}
    </Typography>
    <Typography component="dd" sx={{ m: 0, overflowWrap: 'anywhere' }}>
      {children}
    </Typography>
  </>
);

/** The installed release's identity and where it came from. */
export const ExtensionDetailsSection = ({
  installation,
}: {
  installation: ExtensionInstallation;
}) => {
  const { t } = useTranslation();
  return (
    <Paper sx={{ p: 4 }}>
      <Typography component="h2" sx={{ mb: 2 }} variant="h6">
        {t('extensions.details')}
      </Typography>
      <Box
        component="dl"
        sx={{
          alignItems: 'baseline',
          columnGap: 4,
          display: 'grid',
          gridTemplateColumns: { sm: 'max-content 1fr' },
          m: 0,
          rowGap: 1,
        }}
      >
        <Detail label={t('extensions.detailId')}>
          {installation.extension_id}
        </Detail>
        <Detail label={t('extensions.detailVersion')}>
          {installation.version}
        </Detail>
        <Detail label={t('extensions.detailSource')}>
          {installation.source}
        </Detail>
        <Detail label={t('extensions.detailInstalled')}>
          <Timestamp value={installation.created_at} />
        </Detail>
        <Detail label={t('extensions.detailUpdated')}>
          <Timestamp value={installation.updated_at} />
        </Detail>
        <Detail label={t('extensions.detailManifestHash')}>
          {installation.manifest_sha256}
        </Detail>
      </Box>
    </Paper>
  );
};
