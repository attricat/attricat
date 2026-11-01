import type { BrandVariant } from './brandIconSizes';
import { ArrowLeftIcon, ChevronRightIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Box, Divider, IconButton, List, Typography } from '@mui/material';
import { ExtensionOutlet } from '../features/extensions/ExtensionOutlet';
import { ExploreNavigationLinks } from './ExploreNavigationLinks';
import { ManagementNavigationLinks } from './ManagementNavigationLinks';
import { navigationRoutes } from './navigation';
import { NavigationBrandIcon } from './NavigationBrandIcon';
import { NavigationFlyout } from './NavigationFlyout';
import { NavigationFooter } from './NavigationFooter';
import { NavigationIcon } from './NavigationIcon';
import { NavigationItem } from './NavigationItem';
import { QueryErrorNotice } from './QueryErrorNotice';
import {
  compactNavigationWidth,
  expandedNavigationWidth,
  isWithinRoute,
  mobileExplorePanelId,
  navigationHeaderSx,
  navigationItemHoverSx,
} from './sideNavigationLayout';
import { AppsIcon, ExplorerIcon, ManagementIcon } from './systemIcons';
import { useCompactNavigationPanels } from './useCompactNavigationPanels';
import {
  mobileSectionTitleKeys,
  useMobileNavigationSection,
} from './useMobileNavigationSection';
import { InboxNavigationItem } from './InboxNavigationItem';
import { useSideNavigation } from './useSideNavigation';
import { extensionOutlets } from '../features/extensions/constants';

const navigationSx = {
  ...navigationItemHoverSx,
  display: 'flex',
  flexDirection: 'column',
  height: '100%',
  position: 'relative',
} as const;

const Brand = ({ variant }: { variant: BrandVariant }) => {
  const { t } = useTranslation();
  return (
    <Box
      aria-label={t('app.attricat')}
      data-navigation-icon-trigger
      role="img"
      sx={{ alignItems: 'center', display: 'flex', gap: 1.5 }}
    >
      <NavigationBrandIcon variant={variant} />
    </Box>
  );
};

/**
 * The desktop icon rail. Explore, Apps and Manage open their links in a pane
 * beside it; each pane may be controlled by the layout, which reserves its
 * width.
 */
export const CompactSideNavigation = ({
  exploreOpen,
  extensionsOpen,
  manageOpen,
  onExploreOpenChange,
  onExtensionsOpenChange,
  onManageOpenChange,
  onSignOut,
}: {
  exploreOpen?: boolean;
  extensionsOpen?: boolean;
  manageOpen?: boolean;
  onExploreOpenChange?: (open: boolean) => void;
  onExtensionsOpenChange?: (open: boolean) => void;
  onManageOpenChange?: (open: boolean) => void;
  onSignOut?: () => void;
}) => {
  const { t } = useTranslation();
  const {
    canBrowseExtensions,
    documentationHref,
    exploreLinkProps,
    managementItems,
    pathname,
    pinnedExplore,
    primaryItems,
  } = useSideNavigation();
  const panels = useCompactNavigationPanels(pathname, {
    explore: { onOpenChange: onExploreOpenChange, open: exploreOpen },
    extensions: { onOpenChange: onExtensionsOpenChange, open: extensionsOpen },
    manage: { onOpenChange: onManageOpenChange, open: manageOpen },
  });
  return (
    <Box
      aria-label={t('navigation.main')}
      component="nav"
      sx={{
        ...navigationSx,
        backgroundColor: 'background.navigation.rail',
        // The Explore, Apps and Manage panes sit beside the rail, in the
        // extra width AppLayout allocates, so they must not be clipped.
        overflow: 'visible',
        width: compactNavigationWidth,
      }}
    >
      <Box sx={{ ...navigationHeaderSx, justifyContent: 'center' }}>
        <Brand variant="mark" />
      </Box>
      <Divider />
      <List
        component="div"
        sx={{
          display: 'flex',
          flexDirection: 'column',
          minHeight: 0,
          px: 0.5,
          py: 1.5,
          width: '100%',
        }}
      >
        {primaryItems.map((item) => (
          <NavigationItem
            compact
            icon={<NavigationIcon icon={item.icon} />}
            key={item.to}
            label={t(item.labelKey)}
            onClick={
              item.to === navigationRoutes.explore
                ? () => panels.toggle('explore')
                : panels.closeAll
            }
            selected={
              item.to === navigationRoutes.explore
                ? exploreLinkProps.allRecordsSelected
                : isWithinRoute(pathname, item.to)
            }
            to={item.to}
          />
        ))}
        <Box sx={{ width: '100%' }}>
          <QueryErrorNotice
            error={pinnedExplore.error}
            isRetrying={pinnedExplore.isFetching}
            onRetry={() => void pinnedExplore.refetch()}
          />
        </Box>
        <NavigationItem
          ariaExpanded={panels.isOpen('extensions')}
          compact
          icon={<NavigationIcon icon={AppsIcon} />}
          label={t('navigation.apps')}
          onClick={() => panels.openOnly('extensions')}
          selected={isWithinRoute(
            pathname,
            navigationRoutes.extensionContributions,
          )}
          to={navigationRoutes.extensionContributions}
        />
        <NavigationItem
          ariaExpanded={panels.isOpen('manage')}
          compact
          icon={<NavigationIcon icon={ManagementIcon} />}
          label={t('navigation.manage')}
          onClick={() => panels.openOnly('manage')}
          selected={isWithinRoute(pathname, navigationRoutes.manage)}
          to={navigationRoutes.manage}
        />
      </List>
      {panels.isOpen('manage') && (
        <NavigationFlyout title={t('navigation.manage')}>
          <ManagementNavigationLinks
            items={managementItems}
            pathname={pathname}
          />
        </NavigationFlyout>
      )}
      {panels.isOpen('extensions') && (
        <NavigationFlyout title={t('navigation.apps')}>
          <ExtensionOutlet
            canBrowseExtensions={canBrowseExtensions}
            onBrowseExtensions={() => panels.openOnly('manage')}
            navigationDisplay="all"
            outlet={extensionOutlets.navigation}
          />
        </NavigationFlyout>
      )}
      {panels.isOpen('explore') && (
        <NavigationFlyout title={t('navigation.recordExplorer')}>
          <ExploreNavigationLinks {...exploreLinkProps} />
        </NavigationFlyout>
      )}
      <Box sx={{ flexGrow: 1 }} />
      <NavigationFooter
        compact
        leading={
          <InboxNavigationItem
            compact
            onClick={panels.closeAll}
            pathname={pathname}
          />
        }
        documentationHref={documentationHref}
        onProfileClick={panels.closeAll}
        onSignOut={onSignOut}
        pathname={pathname}
      />
    </Box>
  );
};

/**
 * The mobile drawer: the primary links, with Explore, Apps and Manage each
 * replacing them with their own links and a way back.
 */
export const MobileSideNavigation = ({
  onNavigate,
  onSignOut,
}: {
  /** Called when a link is followed, to close the drawer. */
  onNavigate?: () => void;
  onSignOut?: () => void;
}) => {
  const { t } = useTranslation();
  const {
    canBrowseExtensions,
    documentationHref,
    exploreLinkProps,
    managementItems,
    pathname,
    pinnedExplore,
    primaryItems,
  } = useSideNavigation();
  const [section, setSection] = useMobileNavigationSection(pathname);
  const primary = section === 'primary';
  return (
    <Box
      aria-label={t('navigation.main')}
      component="nav"
      sx={{
        ...navigationSx,
        backgroundColor: 'background.navigation.panel',
        overflow: 'hidden',
        width: expandedNavigationWidth,
      }}
    >
      <Box sx={{ ...navigationHeaderSx, px: primary ? 3 : 1 }}>
        {primary ? (
          <Brand variant="wordmark" />
        ) : (
          <>
            <IconButton
              aria-label={t('navigation.back')}
              onClick={() => setSection('primary')}
            >
              <ArrowLeftIcon />
            </IconButton>
            <Typography sx={{ ml: 1 }} variant="h6">
              {t(mobileSectionTitleKeys[section])}
            </Typography>
          </>
        )}
      </Box>
      <Divider />
      <List
        component="div"
        sx={{
          flexGrow: 1,
          minHeight: 0,
          overflowY: 'auto',
          px: 1,
          py: 1.5,
          width: '100%',
        }}
      >
        {primary && (
          <>
            <NavigationItem
              ariaControls={mobileExplorePanelId}
              ariaExpanded={false}
              compact={false}
              icon={<NavigationIcon icon={ExplorerIcon} />}
              label={t('navigation.recordExplorer')}
              onClick={() => setSection('explore')}
              trailing={<ChevronRightIcon />}
            />
            {primaryItems
              .filter((item) => item.to !== navigationRoutes.explore)
              .map((item) => (
                <NavigationItem
                  compact={false}
                  icon={<NavigationIcon icon={item.icon} />}
                  key={item.to}
                  label={t(item.labelKey)}
                  onClick={onNavigate}
                  selected={isWithinRoute(pathname, item.to)}
                  to={item.to}
                />
              ))}
            <NavigationItem
              compact={false}
              icon={<NavigationIcon icon={AppsIcon} />}
              label={t('navigation.apps')}
              onClick={() => setSection('extensions')}
              sx={{ mt: 1 }}
              to={navigationRoutes.extensionContributions}
              trailing={<ChevronRightIcon />}
            />
            <NavigationItem
              compact={false}
              icon={<NavigationIcon icon={ManagementIcon} />}
              label={t('navigation.manage')}
              onClick={() => setSection('manage')}
              sx={{ mt: 1 }}
              trailing={<ChevronRightIcon />}
            />
          </>
        )}
        {section === 'explore' && (
          <>
            <Box sx={{ px: 1 }}>
              <QueryErrorNotice
                error={pinnedExplore.error}
                isRetrying={pinnedExplore.isFetching}
                onRetry={() => void pinnedExplore.refetch()}
              />
            </Box>
            <Box
              aria-label={t('navigation.recordExplorer')}
              component="nav"
              id={mobileExplorePanelId}
            >
              <List component="div" disablePadding>
                <ExploreNavigationLinks
                  {...exploreLinkProps}
                  onNavigate={onNavigate}
                />
              </List>
            </Box>
          </>
        )}
        {section === 'extensions' && (
          <ExtensionOutlet
            canBrowseExtensions={canBrowseExtensions}
            navigationDisplay="all"
            onNavigate={onNavigate}
            outlet={extensionOutlets.navigation}
          />
        )}
        {section === 'manage' && (
          <ManagementNavigationLinks
            items={managementItems}
            onNavigate={onNavigate}
            pathname={pathname}
          />
        )}
      </List>
      {primary && (
        <NavigationFooter
          compact={false}
          leading={
            <InboxNavigationItem
              compact={false}
              onClick={onNavigate}
              pathname={pathname}
            />
          }
          documentationHref={documentationHref}
          onDocumentationClick={onNavigate}
          onProfileClick={() => onNavigate?.()}
          onSignOut={onSignOut}
          pathname={pathname}
        />
      )}
    </Box>
  );
};
