import { Stack } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { ExtensionContribution } from './api';
import { promotedNavigationGroup } from './constants';
import { ExtensionActionOverflow } from './ExtensionActionOverflow';
import { OutletContribution } from './OutletContribution';
import {
  contributionKey,
  type NavigationPolicy,
  type OutletContext,
} from './outletContributions';

type ExtensionNavigationOutletProps = {
  context?: OutletContext;
  contributions: ExtensionContribution[];
  onNavigate?: () => void;
  policy: NavigationPolicy;
};

/** Shows promoted navigation up to capacity and groups the rest in an overflow. */
export const ExtensionNavigationOutlet = ({
  context,
  contributions,
  onNavigate,
  policy,
}: ExtensionNavigationOutletProps) => {
  const { t } = useTranslation();
  const visible = contributions
    .filter((item) => item.navigation_group === promotedNavigationGroup)
    .slice(0, policy.promotedCapacity);
  const visibleKeys = new Set(visible.map((item) => item.contribution_key));
  const grouped = contributions.filter(
    (item) => !visibleKeys.has(item.contribution_key),
  );
  return (
    <Stack spacing={1}>
      {visible.map((item) => (
        <OutletContribution
          contribution={item}
          context={context}
          key={contributionKey(item)}
          onNavigate={onNavigate}
        />
      ))}
      {grouped.length > 0 && (
        <ExtensionActionOverflow
          context={context}
          contributions={grouped}
          label={t('extensions.groupedNavigation')}
          onNavigate={onNavigate}
        />
      )}
    </Stack>
  );
};
