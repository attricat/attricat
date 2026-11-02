import { Paper, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { Timestamp } from '../../time/Timestamp';
import type { ExtensionInstallation } from './managementApi';
import { Detail, DetailList } from '../../components/DetailList';

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
      <DetailList>
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
      </DetailList>
    </Paper>
  );
};
