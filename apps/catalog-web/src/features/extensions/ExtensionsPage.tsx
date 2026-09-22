import { ExtensionManagementPage } from './ExtensionManagementPage';
import { ExtensionsLayoutPage } from './ExtensionsLayoutPage';

/** @deprecated Use one of the route-specific extension management pages. */
export const ExtensionsPage = () => (
  <ExtensionManagementPage tab={2}>
    <ExtensionsLayoutPage />
  </ExtensionManagementPage>
);
