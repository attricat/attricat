import { describe, expect, it } from 'vitest';
import {
  getVisibleManagementNavigationItems,
  navigationRoutes,
} from './navigation';

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
