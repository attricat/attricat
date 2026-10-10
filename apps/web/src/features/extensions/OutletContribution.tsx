import type { ExtensionContribution } from './api';
import { ExtensionFrame } from './ExtensionFrame';
import { ExtensionNavigationItem } from './ExtensionNavigationItem';
import type { OutletContext } from './outletContributions';

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
  contribution.kind === 'navigation' ? (
    <ExtensionNavigationItem
      contribution={contribution}
      onNavigate={onNavigate}
    />
  ) : (
    <ExtensionFrame contribution={contribution} context={context} />
  );
