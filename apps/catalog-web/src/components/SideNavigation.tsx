import { useQuery } from '@tanstack/react-query';
import { useRouterState } from '@tanstack/react-router';
import { ArrowLeftIcon, ChevronRightIcon } from 'lucide-react';
import { createElement } from 'react';
import { useTranslation } from 'react-i18next';
import { useDocumentationUrl } from '../app/documentation';
import { currentSession } from '../features/auth/api';
import { authQueryKeys } from '../features/auth/queryKeys';
import { ExtensionOutlet } from '../features/extensions/ExtensionOutlet';
import { listSidebarExploreNavigation } from '../features/workspace/api';
import { workspaceQueryKeys } from '../features/workspace/queryKeys';
import { ExploreNavigationLinks } from './ExploreNavigationLinks';
import { ManagementNavigationLinks } from './ManagementNavigationLinks';
import { useSetMobileExplorePanelTarget } from './mobileNavigationPanelContext';
import {
  getVisibleManagementNavigationItems,
  navigationRoutes,
  primaryNavigationItems,
} from './navigation';
import { NavigationFlyout } from './NavigationFlyout';
import { NavigationFooter } from './NavigationFooter';
import { NavigationItem } from './NavigationItem';
import { QueryErrorNotice } from './QueryErrorNotice';
import {
  compactNavigationWidth,
  expandedNavigationWidth,
  isWithinRoute,
  mobileExplorePanelId,
  navigationHeaderSx,
} from './sideNavigationLayout';
import {
  AppsIcon,
  BrandIcon,
  DocumentationIcon,
  ExplorerIcon,
  ManagementIcon,
} from './systemIcons';
import { useCompactNavigationPanels } from './useCompactNavigationPanels';
import {
  mobileSectionTitleKeys,
  useMobileNavigationSection,
} from './useMobileNavigationSection';
import { Box, Divider, IconButton, List, Typography } from '@mui/material';

type SideNavigationProps = {
  compact?: boolean;
  compactManageOpen?: boolean;
  compactExploreOpen?: boolean;
  compactExtensionsOpen?: boolean;
  onCompactManageOpenChange?: (open: boolean) => void;
  onCompactExploreOpenChange?: (open: boolean) => void;
  onCompactExtensionsOpenChange?: (open: boolean) => void;
  onNavigate?: () => void;
  onSignOut?: () => void;
};

export const SideNavigation = ({
  compact = false,
  compactManageOpen,
  compactExploreOpen,
  compactExtensionsOpen,
  onCompactManageOpenChange,
  onCompactExploreOpenChange,
  onCompactExtensionsOpenChange,
  onNavigate,
  onSignOut,
}: SideNavigationProps) => {
  const { t } = useTranslation();
  const setMobileExplorePanelTarget = useSetMobileExplorePanelTarget();
  const documentationUrl = useDocumentationUrl();
  const { pathname, search } = useRouterState({
    select: (state) => state.location,
  });
  const [mobileSection, setMobileSection] =
    useMobileNavigationSection(pathname);
  const panels = useCompactNavigationPanels(pathname, {
    explore: {
      onOpenChange: onCompactExploreOpenChange,
      open: compactExploreOpen,
    },
    extensions: {
      onOpenChange: onCompactExtensionsOpenChange,
      open: compactExtensionsOpen,
    },
    manage: {
      onOpenChange: onCompactManageOpenChange,
      open: compactManageOpen,
    },
  });
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const pinnedExplore = useQuery({
    queryKey: workspaceQueryKeys.sidebarExploreNavigation(),
    queryFn: listSidebarExploreNavigation,
  });
  const managementItems = getVisibleManagementNavigationItems(
    session.data?.capabilities,
  );
  const canBrowseExtensions =
    session.data?.capabilities?.extensions_read === true;
  const exploreActive = pathname === navigationRoutes.explore;
  const exploreLinkProps = {
    allEntitiesSelected: exploreActive && !search.locked,
    selectedBlueprintCode: exploreActive ? search.blueprint : undefined,
    shortcuts: pinnedExplore.data,
  };
  const mobilePrimary = !compact && mobileSection === 'primary';
  const mobileSubNavigationOpen = !compact && mobileSection !== 'primary';
  const navigateAway = () => {
    if (compact) panels.closeAll();
    onNavigate?.();
  };

  return (
    <Box
      sx={{
        backgroundColor: compact ? 'background.default' : 'background.paper',
        display: 'flex',
        flexDirection: 'column',
        height: '100%',
        // Desktop compact navigation renders its Explore/Manage panes beside
        // the icon rail. Let those absolutely positioned panes use the extra
        // width allocated by AppLayout instead of clipping them at the rail.
        overflow: compact ? 'visible' : 'hidden',
        position: 'relative',
        width: compact ? compactNavigationWidth : expandedNavigationWidth,
      }}
    >
      <Box
        sx={{
          ...navigationHeaderSx,
          justifyContent: compact ? 'center' : undefined,
          px: compact ? undefined : mobileSubNavigationOpen ? 1 : 3,
        }}
      >
        {mobileSubNavigationOpen ? (
          <>
            <IconButton
              aria-label={t('navigation.back')}
              onClick={() => setMobileSection('primary')}
            >
              <ArrowLeftIcon />
            </IconButton>
            <Typography sx={{ ml: 1 }} variant="h6">
              {t(mobileSectionTitleKeys[mobileSection])}
            </Typography>
          </>
        ) : (
          <Box
            aria-label={t('app.attricat')}
            sx={{ alignItems: 'center', display: 'flex', gap: 1.5 }}
          >
            <BrandIcon variant={compact ? 'mark' : 'wordmark'} />
          </Box>
        )}
      </Box>
      <Divider />
      <List
        sx={{
          display: compact ? 'flex' : 'block',
          flexDirection: compact ? 'column' : undefined,
          flexGrow: compact ? 0 : 1,
          minHeight: 0,
          overflowY: compact ? 'visible' : 'auto',
          px: compact ? 0.5 : 1,
          py: 1.5,
          width: '100%',
        }}
      >
        {mobilePrimary && (
          <NavigationItem
            ariaControls={mobileExplorePanelId}
            ariaExpanded={false}
            compact={false}
            icon={<ExplorerIcon />}
            label={t('navigation.entityExplorer')}
            onClick={() => setMobileSection('explore')}
            trailing={<ChevronRightIcon />}
          />
        )}
        {(compact || mobilePrimary) &&
          primaryNavigationItems
            .filter((item) => compact || item.to !== navigationRoutes.explore)
            .map((item) => (
              <NavigationItem
                compact={compact}
                icon={createElement(item.icon)}
                key={item.to}
                label={t(item.labelKey)}
                onClick={
                  compact && item.to === navigationRoutes.explore
                    ? () => panels.toggle('explore')
                    : navigateAway
                }
                selected={
                  item.to === navigationRoutes.explore
                    ? exploreLinkProps.allEntitiesSelected
                    : pathname.startsWith(item.to)
                }
                to={item.to}
              />
            ))}
        {(compact || mobileSection === 'explore') && (
          <Box sx={compact ? { width: '100%' } : { px: 1 }}>
            <QueryErrorNotice
              error={pinnedExplore.error}
              isRetrying={pinnedExplore.isFetching}
              onRetry={() => void pinnedExplore.refetch()}
            />
          </Box>
        )}
        {!compact && mobileSection === 'explore' && (
          <Box
            aria-label={t('navigation.entityExplorer')}
            component="nav"
            id={mobileExplorePanelId}
          >
            <List disablePadding>
              <ExploreNavigationLinks
                {...exploreLinkProps}
                onNavigate={onNavigate}
              />
            </List>
            <Box ref={setMobileExplorePanelTarget} sx={{ mt: 2 }} />
          </Box>
        )}
        {compact && (
          <NavigationItem
            ariaExpanded={panels.isOpen('extensions')}
            compact
            icon={<AppsIcon />}
            label={t('navigation.apps')}
            onClick={() => panels.openOnly('extensions')}
            selected={isWithinRoute(
              pathname,
              navigationRoutes.extensionContributions,
            )}
            to={navigationRoutes.extensionContributions}
          />
        )}
        {mobilePrimary && (
          <NavigationItem
            compact={false}
            icon={<AppsIcon />}
            label={t('navigation.apps')}
            onClick={() => setMobileSection('extensions')}
            sx={{ mt: 1 }}
            to={navigationRoutes.extensionContributions}
            trailing={<ChevronRightIcon />}
          />
        )}
        {!compact && mobileSection === 'extensions' && (
          <ExtensionOutlet
            canBrowseExtensions={canBrowseExtensions}
            navigationDisplay="all"
            onNavigate={onNavigate}
            outlet="navigation"
          />
        )}
        {compact && (
          <NavigationItem
            ariaExpanded={panels.isOpen('manage')}
            compact
            icon={<ManagementIcon />}
            label={t('navigation.manage')}
            onClick={() => panels.openOnly('manage')}
            selected={isWithinRoute(pathname, navigationRoutes.manage)}
            to={navigationRoutes.manage}
          />
        )}
        {mobilePrimary && (
          <NavigationItem
            compact={false}
            icon={<ManagementIcon />}
            label={t('navigation.manage')}
            onClick={() => setMobileSection('manage')}
            sx={{ mt: 1 }}
            trailing={<ChevronRightIcon />}
          />
        )}
        {(compact || mobilePrimary) && (
          <NavigationItem
            compact={compact}
            href={documentationUrl('home')}
            icon={<DocumentationIcon />}
            label={t('navigation.documentation')}
            onClick={compact ? undefined : onNavigate}
            sx={{ mt: 1 }}
          />
        )}
        {!compact && mobileSection === 'manage' && (
          <ManagementNavigationLinks
            items={managementItems}
            onNavigate={onNavigate}
            pathname={pathname}
          />
        )}
      </List>
      {compact && panels.isOpen('manage') && (
        <NavigationFlyout title={t('navigation.manage')}>
          <ManagementNavigationLinks
            items={managementItems}
            onNavigate={onNavigate}
            pathname={pathname}
          />
        </NavigationFlyout>
      )}
      {compact && panels.isOpen('extensions') && (
        <NavigationFlyout title={t('navigation.apps')}>
          <ExtensionOutlet
            canBrowseExtensions={canBrowseExtensions}
            onBrowseExtensions={() => panels.openOnly('manage')}
            navigationDisplay="all"
            onNavigate={onNavigate}
            outlet="navigation"
          />
        </NavigationFlyout>
      )}
      {compact && panels.isOpen('explore') && (
        <NavigationFlyout title={t('navigation.entityExplorer')}>
          <ExploreNavigationLinks {...exploreLinkProps} />
          <Box ref={setMobileExplorePanelTarget} sx={{ mt: 2 }} />
        </NavigationFlyout>
      )}
      {compact && <Box sx={{ flexGrow: 1 }} />}
      {(compact || mobilePrimary) && (
        <NavigationFooter
          compact={compact}
          onProfileClick={navigateAway}
          onSignOut={onSignOut}
          pathname={pathname}
        />
      )}
    </Box>
  );
};
