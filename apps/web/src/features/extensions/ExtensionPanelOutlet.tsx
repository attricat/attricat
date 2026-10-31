import { Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { ExtensionContribution } from './api';
import { ExtensionActionOverflow } from './ExtensionActionOverflow';
import { ExtensionUi } from './ExtensionUi';
import {
  contributionKey,
  type OutletContext,
  type PanelPolicy,
} from './outletContributions';

type ExtensionPanelOutletProps = {
  context?: OutletContext;
  contributions: ExtensionContribution[];
  policy: PanelPolicy;
  /** What the outlet's contributions belong to, such as an attribute. */
  heading?: string;
};

export const ExtensionPanelOutlet = ({
  context,
  contributions,
  policy,
  heading,
}: ExtensionPanelOutletProps) => {
  const { t } = useTranslation();
  const visible = contributions.slice(0, policy.visibleCapacity);
  const overflow = contributions.slice(policy.visibleCapacity);
  return (
    <Stack spacing={1}>
      {visible.map((item) => (
        <ExtensionUi
          contribution={item}
          context={context}
          heading={
            heading && (
              <Typography component="h3" sx={{ mb: 1 }} variant="subtitle2">
                {t('extensions.contributionHeading', {
                  subject: heading,
                  extension: item.extension_name,
                })}
              </Typography>
            )
          }
          key={contributionKey(item)}
          placeholder={policy.placeholder}
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
