import { Box, Paper, Typography } from '@mui/material';
import { Trans, useTranslation } from 'react-i18next';
import { UserLabel } from '../../../components/UserLabel';
import type { RecordAuditChange } from '../api';
import { AGENT_EXECUTOR_TYPE } from '../constants';
import { Timestamp } from '../../../time/Timestamp';

type Props = {
  /** Changes recorded by one audit event; the first describes the event. */
  changes: readonly [RecordAuditChange, ...RecordAuditChange[]];
};

/** One audit event with the attribute changes it recorded. */
export const RecordChangeEvent = ({ changes }: Props) => {
  const { t } = useTranslation();
  const [event] = changes;
  const actorName = event.actor_display_name ?? event.actor_email;
  const actor = actorName ? (
    <UserLabel avatarFileId={event.actor_avatar_file_id} name={actorName} />
  ) : event.executor_type === AGENT_EXECUTOR_TYPE ? (
    t('records.agent')
  ) : (
    t('records.unknownActor')
  );
  const approval = event.approval_decision ? (
    event.approved_by_display_name ? (
      <Trans
        components={{
          actor: (
            <UserLabel
              avatarFileId={event.approved_by_avatar_file_id}
              name={event.approved_by_display_name}
            />
          ),
        }}
        i18nKey="records.approvalBy"
        t={t}
        values={{ decision: event.approval_decision }}
      />
    ) : (
      t('records.approval', { decision: event.approval_decision })
    )
  ) : (
    t('records.noApproval')
  );
  return (
    <Paper component="section" sx={{ mb: 2, p: 2 }}>
      <Typography sx={{ fontWeight: 'bold' }}>
        {actor} ·{' '}
        <Timestamp style="dateTimeSeconds" value={event.occurred_at} />
      </Typography>
      <Typography color="text.secondary" variant="body2">
        {approval}
      </Typography>
      {changes.map((change) => (
        <Box
          key={`${change.attribute_id}-${change.context_id}-${change.change_kind}`}
          sx={{ mt: 1 }}
        >
          <Typography variant="body2">
            <strong>{change.attribute_code}</strong>
            {change.context_code ? ` (${change.context_code})` : ''}:{' '}
            {t(`records.changeKind.${change.change_kind}`)}
          </Typography>
          <Typography
            component="pre"
            sx={{ fontFamily: 'monospace', m: 0, whiteSpace: 'pre-wrap' }}
            variant="body2"
          >
            {JSON.stringify(change.before_value)} →{' '}
            {JSON.stringify(change.after_value)}
          </Typography>
        </Box>
      ))}
    </Paper>
  );
};
