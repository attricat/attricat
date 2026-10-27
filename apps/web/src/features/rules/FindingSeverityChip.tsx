import { Chip, type SxProps, type Theme } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { Finding } from './api';
import { findingSeverityColor } from './findingSeverity';

/** A finding's translated severity, coloured the same on every screen. */
export const FindingSeverityChip = ({
  severity,
  sx,
}: {
  severity: Finding['severity'];
  sx?: SxProps<Theme>;
}) => {
  const { t } = useTranslation();
  return (
    <Chip
      color={findingSeverityColor(severity)}
      label={t(`rules.severities.${severity}`)}
      size="small"
      sx={sx}
    />
  );
};
