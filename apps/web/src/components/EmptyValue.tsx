import { Tooltip, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { VisuallyHidden } from './VisuallyHidden';

const placeholder = '—';

/**
 * A missing value: a muted dash, named on hover and for assistive technology.
 * The dash stays out of reach of screen readers, which read the hidden label
 * instead. `label` replaces the generic "Not set" where the field can say
 * more, such as that no files were added.
 */
export const EmptyValue = ({ label: override }: { label?: string } = {}) => {
  const { t } = useTranslation();
  const label = override ?? t('views.notSet');
  return (
    // The hidden label already names it, so the tooltip only describes.
    <Tooltip describeChild title={label}>
      <Typography
        color="text.secondary"
        component="span"
        // Matches the text around it; a wider target than the dash alone that keeps its place in the layout.
        sx={{ mx: -0.5, px: 0.5 }}
        variant="inherit"
      >
        <span aria-hidden="true">{placeholder}</span>
        <VisuallyHidden>{label}</VisuallyHidden>
      </Typography>
    </Tooltip>
  );
};
