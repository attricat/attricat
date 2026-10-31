import type { ExtensionContribution } from './api';
import { ExtensionUi } from './ExtensionUi';
import { ExtensionNavigationItem } from './ExtensionNavigationItem';
import type { OutletContext } from './outletContributions';
import { contributionKinds } from './constants';

type OutletContributionProps = {
  contribution: ExtensionContribution;
  context?: OutletContext;
  onNavigate?: () => void;
};

/** Renders host navigation for navigation contributions and a frame otherwise. */
export const OutletContribution = ({
  contribution,
  context,
  onNavigate,
}: OutletContributionProps) =>
  contribution.kind === contributionKinds.navigation ? (
    <ExtensionNavigationItem
      contribution={contribution}
      onNavigate={onNavigate}
    />
  ) : (
    <ExtensionUi contribution={contribution} context={context} />
  );
