import { z } from 'zod';
import i18n from '../../i18n';
import { toast } from '../../components/toast';
import {
  type BrowserAuthorizationOutcome,
  browserAuthorizationOutcomes,
} from './constants';

/** The query the public OAuth callback appends when it returns the browser. */
export const browserAuthorizationSearchSchema = z.object({
  authorization: z
    .enum(browserAuthorizationOutcomes)
    .optional()
    .catch(undefined),
  connection: z.string().max(128).optional().catch(undefined),
});

const severities = {
  connected: 'success',
  failed: 'error',
  expired: 'warning',
  signed_out: 'warning',
} as const satisfies Record<BrowserAuthorizationOutcome, string>;

/** Tells the operator how a browser authorization ended. */
export const showBrowserAuthorizationNotice = (
  outcome: BrowserAuthorizationOutcome,
  connection = '',
) =>
  toast.show({
    message: i18n.t(`extensionExports.authorizationOutcome.${outcome}`, {
      connection,
    }),
    severity: severities[outcome],
  });
