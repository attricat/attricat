import { useQuery } from '@tanstack/react-query';
import { useRouterState } from '@tanstack/react-router';
import { useDocumentationUrl } from '../app/documentation';
import { useCan } from '../features/auth/useCan';
import { sessionOptions } from '../features/auth/queryOptions';
import { capabilities } from '../features/auth/capabilities';
import { listSidebarExploreNavigation } from '../features/workspace/api';
import { workspaceQueryKeys } from '../features/workspace/queryKeys';
import {
  getVisibleManagementNavigationItems,
  getVisiblePrimaryNavigationItems,
  navigationRoutes,
} from './navigation';

/** What the compact rail and the mobile drawer both show. */
export const useSideNavigation = () => {
  const documentationUrl = useDocumentationUrl();
  // Only what the links show, so other search changes do not re-render it.
  const { pathname, search } = useRouterState({
    select: ({ location }) => ({
      pathname: location.pathname,
      search: {
        blueprint: location.search.blueprint,
        locked: location.search.locked,
      },
    }),
    structuralSharing: true,
  });
  const session = useQuery(sessionOptions());
  const pinnedExplore = useQuery({
    queryKey: workspaceQueryKeys.sidebarExploreNavigation(),
    queryFn: listSidebarExploreNavigation,
  });
  const canBrowseExtensions = useCan(capabilities.extensionsRead);
  const exploreActive = pathname === navigationRoutes.explore;
  return {
    canBrowseExtensions,
    documentationHref: documentationUrl('home'),
    exploreLinkProps: {
      allRecordsSelected: exploreActive && !search.locked,
      selectedBlueprintCode: exploreActive ? search.blueprint : undefined,
      shortcuts: pinnedExplore.data,
    },
    managementItems: getVisibleManagementNavigationItems(
      session.data?.capabilities,
    ),
    pathname,
    pinnedExplore,
    primaryItems: getVisiblePrimaryNavigationItems(session.data?.capabilities),
  };
};
