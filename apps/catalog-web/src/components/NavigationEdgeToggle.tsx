import { IconButton, Tooltip } from '@mui/material';
import { ChevronLeftIcon, ChevronRightIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from './iconSizes';
import {
  navigationEdgeToggleSize,
  navigationHeaderHeight,
} from './sideNavigationLayout';

// Small chevron straddling the right edge of the desktop navigation, centred
// on the header divider. Rendered outside the drawer, which clips overflow.
export const NavigationEdgeToggle = ({
  edge,
  expanded,
  onToggle,
}: {
  edge: number;
  expanded: boolean;
  onToggle: () => void;
}) => {
  const { t } = useTranslation();
  const label = t(
    expanded ? 'navigation.collapsePanel' : 'navigation.expandPanel',
  );
  const Icon = expanded ? ChevronLeftIcon : ChevronRightIcon;

  return (
    <Tooltip placement="right" title={label}>
      <IconButton
        aria-expanded={expanded}
        aria-label={label}
        onClick={onToggle}
        sx={{
          '&:hover': {
            backgroundColor: 'background.paper',
            color: 'primary.main',
          },
          backgroundColor: 'background.paper',
          border: 1,
          borderColor: 'divider',
          borderRadius: '50%',
          boxShadow: 1,
          color: 'text.secondary',
          height: navigationEdgeToggleSize,
          left: edge - navigationEdgeToggleSize / 2,
          p: 0,
          position: 'fixed',
          top: navigationHeaderHeight - navigationEdgeToggleSize / 2,
          width: navigationEdgeToggleSize,
          zIndex: (theme) => theme.zIndex.drawer + 1,
        }}
      >
        <Icon size={compactIconSize} />
      </IconButton>
    </Tooltip>
  );
};
