import { AppBar, Box, Drawer, IconButton, Toolbar } from '@mui/material';
import { MenuIcon } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { BrandIcon } from './BrandIcon';
import { SideNavigation } from './SideNavigation';
import { expandedNavigationWidth } from './sideNavigationLayout';

export const MobileNavigation = ({
  onSignOut,
  pathname,
}: {
  onSignOut: () => void;
  pathname: string;
}) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const close = () => setOpen(false);

  return (
    <>
      <AppBar position="fixed">
        <Toolbar>
          <IconButton
            aria-label={t('navigation.open')}
            color="inherit"
            edge="start"
            onClick={() => setOpen(true)}
          >
            <MenuIcon />
          </IconButton>
          <Box aria-label={t('app.attricat')} sx={{ flexGrow: 1, ml: 1 }}>
            <BrandIcon variant="wordmark" />
          </Box>
        </Toolbar>
      </AppBar>
      <Drawer
        onClose={close}
        open={open}
        slotProps={{ paper: { sx: { width: expandedNavigationWidth } } }}
        variant="temporary"
      >
        <SideNavigation
          key={`${pathname}:${open ? 'open' : 'closed'}`}
          onNavigate={close}
          onSignOut={onSignOut}
        />
      </Drawer>
    </>
  );
};
