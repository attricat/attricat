import { Box, Drawer, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { RouterButton } from '../../components/RouterLink';
import { ExtensionOutlet } from '../extensions/ExtensionOutlet';
import type { AuditEvent } from './api';
import { auditActor, formatAuditDate } from './auditFormat';
import {
  auditDrawerWidth,
  auditEventPanelOutlet,
  auditEventTitleId,
  auditExtensionContextVersion,
} from './constants';

const Detail = ({ label, value }: { label: string; value: unknown }) => (
  <Box>
    <Typography color="text.secondary" variant="caption">
      {label}
    </Typography>
    <Typography
      component="pre"
      sx={{ m: 0, overflowWrap: 'anywhere', whiteSpace: 'pre-wrap' }}
      variant="body2"
    >
      {typeof value === 'string' ? value : JSON.stringify(value, null, 2)}
    </Typography>
  </Box>
);

export const AuditEventDrawer = ({
  event,
  locale,
  onClose,
  systemLabel,
}: {
  event?: AuditEvent;
  locale: string;
  onClose: () => void;
  systemLabel: string;
}) => {
  const { t } = useTranslation();
  const titleId = event ? auditEventTitleId(event.id) : undefined;
  return (
    <Drawer
      anchor="right"
      aria-labelledby={titleId}
      onClose={onClose}
      open={Boolean(event)}
    >
      <Box sx={{ p: 3, width: { xs: '100vw', sm: auditDrawerWidth } }}>
        <Typography component="h2" id={titleId} variant="h6">
          {t('audit.event')}
        </Typography>
        {event && (
          <Box sx={{ display: 'grid', gap: 2, mt: 2 }}>
            <Typography>
              {t('audit.eventBy', {
                action: event.action,
                actor: auditActor(event, systemLabel),
                date: formatAuditDate(event.occurred_at, locale),
              })}
            </Typography>
            {event.agent_conversation_id && (
              <RouterButton
                params={{ conversationId: event.agent_conversation_id }}
                to="/agents/$conversationId"
              >
                {t('audit.openConversation')}
              </RouterButton>
            )}
            <Detail
              label={t('audit.authorizationScope')}
              value={event.authorization_scope}
            />
            <Detail label={t('audit.target')} value={event.target} />
            <Detail label={t('audit.safeMetadata')} value={event.metadata} />
            <Detail label={t('audit.requestId')} value={event.request_id} />
            <Detail
              label={t('audit.correlationId')}
              value={event.correlation_id}
            />
            {event.approval_decision && (
              <Detail
                label={t('audit.agentApproval')}
                value={t('audit.approvalDecisionBy', {
                  decision: event.approval_decision,
                  approver:
                    event.approved_by_display_name ??
                    event.approved_by_email ??
                    event.approved_by_user_id,
                })}
              />
            )}
            {event.agent_tool_call_id && (
              <Detail
                label={t('audit.agentToolCall')}
                value={`${event.agent_tool_name}: ${event.agent_tool_call_id}`}
              />
            )}
            <ExtensionOutlet
              context={{
                context_version: auditExtensionContextVersion,
                event_id: event.id,
              }}
              outlet={auditEventPanelOutlet}
            />
          </Box>
        )}
      </Box>
    </Drawer>
  );
};
