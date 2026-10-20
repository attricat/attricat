import { ExternalLinkIcon } from 'lucide-react';

// A hint beside text, smaller than the icons of dense controls.
const newTabIconSize = 14;

/** Marks a link that opens in a new tab, ahead of its text. */
export const NewTabIcon = ({ size = newTabIconSize }: { size?: number }) => (
  <ExternalLinkIcon aria-hidden size={size} />
);
