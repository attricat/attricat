import { Divider, Stack } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { ExtensionContribution } from './api';
import { ExtensionActionOverflow } from './ExtensionActionOverflow';
import { ExtensionFrame } from './ExtensionFrame';
import {
  contributionKey,
  type ActionBarPolicy,
  type OutletContext,
} from './outletContributions';

type ExtensionActionBarOutletProps = {
  context?: OutletContext;
  contributions: ExtensionContribution[];
  policy: ActionBarPolicy;
};

export const ExtensionActionBarOutlet = ({
  context,
  contributions,
  policy,
}: ExtensionActionBarOutletProps) => {
  const { t } = useTranslation();
  const visibleCapacity = policy.primaryCapacity + policy.secondaryCapacity;
  const primary = contributions.slice(0, policy.primaryCapacity);
  const secondary = contributions.slice(
    policy.primaryCapacity,
    visibleCapacity,
  );
  const overflow = contributions.slice(visibleCapacity);
  return (
    <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
      {primary.map((item) => (
        <ExtensionFrame
          contribution={item}
          context={context}
          key={contributionKey(item)}
        />
      ))}
      {secondary.length > 0 && primary.length > 0 && (
        <Divider flexItem orientation="vertical" />
      )}
      {secondary.map((item) => (
        <ExtensionFrame
          contribution={item}
          context={context}
          key={contributionKey(item)}
        />
      ))}
      {overflow.length > 0 && (
        <ExtensionActionOverflow
          context={context}
          contributions={overflow}
          label={t('extensions.moreActions')}
        />
      )}
    </Stack>
  );
};
