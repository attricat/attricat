import { Stack } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { ExtensionContribution } from './api';
import { ExtensionActionOverflow } from './ExtensionActionOverflow';
import { ExtensionFrame } from './ExtensionFrame';
import {
  contributionKey,
  type ContentPolicy,
  type OutletContext,
} from './outletContributions';

type ExtensionPanelOutletProps = {
  context?: OutletContext;
  contributions: ExtensionContribution[];
  policy: ContentPolicy;
};

export const ExtensionPanelOutlet = ({
  context,
  contributions,
  policy,
}: ExtensionPanelOutletProps) => {
  const { t } = useTranslation();
  const visible = contributions.slice(0, policy.visibleCapacity);
  const overflow = contributions.slice(policy.visibleCapacity);
  return (
    <Stack spacing={1}>
      {visible.map((item) => (
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
          label={t('extensions.moreContent')}
        />
      )}
    </Stack>
  );
};
