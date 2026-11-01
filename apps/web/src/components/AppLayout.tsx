import { rememberReturnLocation } from '../features/auth/returnLocation';
import {
  Navigate,
  Outlet,
  useNavigate,
  useRouterState,
} from '@tanstack/react-router';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Box,
  CircularProgress,
  Toolbar,
  useMediaQuery,
  useTheme,
} from '@mui/material';
import { Fragment, useEffect, useLayoutEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { pageTitle } from '../app/pageTitle';
import { logout } from '../features/auth/api';
import { authQueryKeys } from '../features/auth/queryKeys';
import { ExtensionRunWatcher } from '../features/extension-runs/ExtensionRunWatcher';
import { ExtensionActionDialogHost } from '../features/extensions/ExtensionActionDialogHost';
import { ExtensionActionConfirmHost } from '../features/extensions/ExtensionActionConfirmHost';
import { NativeWarmup } from '../features/extensions/native/NativeWarmup';
import { useWorkspaceLexicon } from '../features/lexicon/lexicon';
import { useSessionCacheBoundary } from '../features/auth/useSessionCacheBoundary';
import { DesktopNavigation } from './DesktopNavigation';
import { MobileNavigation } from './MobileNavigation';
import { navigationRoutes, publicRoutes } from './navigation';
import { SessionErrorState, SignOutErrorState } from './SessionErrorStates';
import { isWithinRoute } from './sideNavigationLayout';
import { sessionOptions } from '../features/auth/queryOptions';
import { ActionPalette } from '../features/extensions/ActionPalette';
import { mainContentId, SkipToContentLink } from './SkipToContentLink';

const signOutErrorMaxWidth = 480;

// Rendered before the login redirect so its effect records the original
// location before navigation replaces it.
const RememberReturnLocation = () => {
  useEffect(rememberReturnLocation, []);
  return null;
};

export const AppLayout = () => {
  const { t } = useTranslation();
  const pathname = useRouterState({
    select: (state) => state.location.pathname,
  });
  useLayoutEffect(() => {
    document.title = pageTitle(pathname, t);
  }, [pathname, t]);
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const session = useQuery({
    ...sessionOptions(),
    retry: false,
  });
  useWorkspaceLexicon(session.data?.workspace_id);
  const sessionBoundary = useSessionCacheBoundary(
    session.data,
    session.isSuccess,
  );
  const theme = useTheme();
  const isDesktop = useMediaQuery(theme.breakpoints.up('md'));
  const [signOutError, setSignOutError] = useState(false);
  const completeSignOut = async () => {
    queryClient.clear();
    queryClient.setQueryData(authQueryKeys.session(), null);
    await navigate({ to: publicRoutes.login });
  };
  const signOut = async () => {
    try {
      await logout();
    } catch {
      setSignOutError(true);
      return;
    }
    await completeSignOut();
  };
  const signOutAfterSessionFailure = async () => {
    try {
      await logout();
    } catch {
      // The login route remains available even if the invalid session cannot be cleared.
    }
    await completeSignOut();
  };
  const isLoginRoute = isWithinRoute(pathname, publicRoutes.login);
  if (
    isWithinRoute(pathname, publicRoutes.passwordReset) ||
    pathname === publicRoutes.onboarding
  )
    return <Outlet />;
  if (session.isPending || (session.isSuccess && !sessionBoundary.ready))
    return (
      <Box
        aria-label={t('app.loading')}
        sx={{
          alignItems: 'center',
          display: 'flex',
          height: '100dvh',
          justifyContent: 'center',
        }}
      >
        <CircularProgress enableTrackSlot />
      </Box>
    );
  if (isLoginRoute)
    return session.data ? (
      <Navigate to={navigationRoutes.explore} />
    ) : (
      <Outlet />
    );
  if (session.isError)
    return (
      <SessionErrorState
        onRetry={() => void session.refetch()}
        onSignOut={() => void signOutAfterSessionFailure()}
      />
    );
  if (!session.data)
    return (
      <>
        <RememberReturnLocation />
        <Navigate to={publicRoutes.login} />
      </>
    );

  return (
    <Fragment key={sessionBoundary.identity}>
      <Box sx={{ display: 'flex', minHeight: '100dvh' }}>
        <SkipToContentLink />
        {signOutError && (
          <Box
            sx={{
              left: (theme) => theme.spacing(6),
              maxWidth: signOutErrorMaxWidth,
              position: 'fixed',
              top: (theme) => theme.spacing(6),
              zIndex: (theme) => theme.zIndex.snackbar,
            }}
          >
            <SignOutErrorState onRetry={() => void signOut()} />
          </Box>
        )}
        {isDesktop ? (
          <DesktopNavigation onSignOut={signOut} pathname={pathname} />
        ) : (
          <MobileNavigation onSignOut={signOut} pathname={pathname} />
        )}
        <Box
          component="main"
          id={mainContentId}
          // Focusable for the skip link; the landmark itself shows no ring.
          sx={{ flexGrow: 1, minWidth: 0, '&:focus': { outline: 'none' } }}
          tabIndex={-1}
        >
          {/* Offsets content below the fixed mobile app bar at every toolbar height. */}
          {!isDesktop && <Toolbar aria-hidden />}
          <Outlet />
        </Box>
      </Box>
      <ExtensionActionDialogHost />
      <ExtensionActionConfirmHost />
      <ActionPalette />
      <ExtensionRunWatcher />
      <NativeWarmup />
    </Fragment>
  );
};
