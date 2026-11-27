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
    // The hidden label already names it. Tooltip would also set `title`, or
    // `aria-describedby` while open, which some screen readers announce on
    // top of the label; the child's own props override both.
    <Tooltip describeChild title={label}>
      <Typography
        aria-describedby={undefined}
        title={undefined}
        color="text.secondary"
        component="span"
        // Matches the text around it; a wider target than the dash alone that keeps its place in the layout.
        // Positioned so the hidden label stays inside the cell: otherwise it
        // escapes a horizontally scrolled table and widens the page.
        sx={{ mx: -0.5, position: 'relative', px: 0.5 }}
        variant="inherit"
      >
        <span aria-hidden="true">{placeholder}</span>
        <VisuallyHidden>{label}</VisuallyHidden>
      </Typography>
    </Tooltip>
  );
};
