import { describe, expect, it } from 'vitest';
import {
  getVisibleManagementNavigationItems,
  getVisiblePrimaryNavigationItems,
  navigationRoutes,
} from './navigation';

const routes = (items: readonly { to: string }[]) =>
  items.map((item) => item.to);

describe('getVisibleManagementNavigationItems', () => {
  it('shows background processing only with data-health read access', () => {
    for (const capabilities of [undefined, { data_health_read: false }]) {
      expect(
        getVisibleManagementNavigationItems(capabilities).map(
          (item) => item.to,
        ),
      ).not.toContain(navigationRoutes.backgroundProcessing);
    }
    expect(
      getVisibleManagementNavigationItems({ data_health_read: true }).map(
        (item) => item.to,
      ),
    ).toContain(navigationRoutes.backgroundProcessing);
  });
  it('shows data health only with data-health read access', () => {
    expect(
      routes(getVisibleManagementNavigationItems({ data_health_read: false })),
    ).not.toContain(navigationRoutes.dataHealth);
    expect(
      routes(getVisibleManagementNavigationItems({ data_health_read: true })),
    ).toContain(navigationRoutes.dataHealth);
  });
  it('shows the workspace to managers of its sections only', () => {
    expect(
      routes(
        getVisibleManagementNavigationItems({ tokens_manage: true } as never),
      ),
    ).not.toContain(navigationRoutes.workspace);
    expect(
      routes(
        getVisibleManagementNavigationItems({
          workspace_navigation_manage: true,
        }),
      ),
    ).toContain(navigationRoutes.workspace);
  });
  it('shows system health only with data-health read access', () => {
    expect(
      getVisibleManagementNavigationItems({ data_health_read: false }).map(
        (item) => item.to,
      ),
    ).not.toContain(navigationRoutes.systemHealth);
    expect(
      getVisibleManagementNavigationItems({ data_health_read: true }).map(
        (item) => item.to,
      ),
    ).toContain(navigationRoutes.systemHealth);
  });
  it('shows workflows only to users with workflow read access', () => {
    expect(
      getVisibleManagementNavigationItems({ workflows_read: false }).map(
        (item) => item.to,
      ),
    ).not.toContain(navigationRoutes.workflows);

    expect(
      getVisibleManagementNavigationItems({ workflows_read: true }).map(
        (item) => item.to,
      ),
    ).toContain(navigationRoutes.workflows);
  });
});

describe('getVisiblePrimaryNavigationItems', () => {
  it('shows agents only to users who may run them', () => {
    for (const capabilities of [undefined, { agents_run: false }])
      expect(
        routes(getVisiblePrimaryNavigationItems(capabilities)),
      ).not.toContain(navigationRoutes.agents);
    expect(
      routes(getVisiblePrimaryNavigationItems({ agents_run: true })),
    ).toContain(navigationRoutes.agents);
  });
});
