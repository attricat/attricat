import { Link } from '@mui/material';
import type { ReactNode } from 'react';

/** A field's label in running text that leads to the field. */
export const FieldRevealLink = ({
  children,
  onClick,
}: {
  children: ReactNode;
  onClick: () => void;
}) => (
  <Link
    component="button"
    onClick={onClick}
    sx={{ font: 'inherit', verticalAlign: 'baseline' }}
    type="button"
  >
    {children}
  </Link>
);
