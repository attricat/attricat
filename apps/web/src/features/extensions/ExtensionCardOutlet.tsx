import { Paper, Stack } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { ExtensionContribution } from './api';
import { extensionCardMinWidth } from './constants';
import { ExtensionActionOverflow } from './ExtensionActionOverflow';
import { ExtensionFrame } from './ExtensionFrame';
import {
  contributionKey,
  type ContentPolicy,
  type OutletContext,
} from './outletContributions';

type ExtensionCardOutletProps = {
  context?: OutletContext;
  contributions: ExtensionContribution[];
  policy: ContentPolicy;
};

export const ExtensionCardOutlet = ({
  context,
  contributions,
  policy,
}: ExtensionCardOutletProps) => {
  const { t } = useTranslation();
  const visible = contributions.slice(0, policy.visibleCapacity);
  const overflow = contributions.slice(policy.visibleCapacity);
  return (
    <Stack
      direction={{ xs: 'column', sm: 'row' }}
      spacing={2}
      sx={{ flexWrap: 'wrap' }}
    >
      {visible.map((item) => (
        <Paper
          key={contributionKey(item)}
          sx={{ minWidth: extensionCardMinWidth, p: 2 }}
        >
          <ExtensionFrame contribution={item} context={context} />
        </Paper>
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
