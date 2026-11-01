import { personName } from '../../../components/personName';
import { Box, Paper, Typography } from '@mui/material';
import { Trans, useTranslation } from 'react-i18next';
import { JsonInline } from '../../../components/JsonValue';
import { UserLabel } from '../../../components/UserLabel';
import type { Attribute, RecordAuditChange } from '../api';
import { AttributeValueText } from '../../views/components/values/AttributeValueText';
import { attributeValueTypes } from '../valueTypes';
import { relationshipChangeKinds } from '../recordChanges';
import { executorTypes } from '../../audit/constants';
import { Timestamp } from '../../../time/Timestamp';

// Value types whose values are objects that the attribute formats itself.
const objectValueTypes: ReadonlySet<string> = new Set([
  attributeValueTypes.json,
  attributeValueTypes.time,
]);

/**
 * A changed value as the record shows it, such as a choice's label. Values
 * the attribute cannot format, because it was removed or changed type since,
 * show as JSON.
 */
const ChangeValue = ({
  attribute,
  value,
}: {
  attribute?: Attribute;
  value: unknown;
}) =>
  attribute &&
  (value === null ||
    typeof value !== 'object' ||
    objectValueTypes.has(attribute.value_type)) ? (
    <AttributeValueText attribute={attribute} value={value} />
  ) : (
    <JsonInline value={value} />
  );

type Props = {
  /** Changes recorded by one audit event; the first describes the event. */
  changes: readonly [RecordAuditChange, ...RecordAuditChange[]];
  /** The attribute's name, or its code when the revision does not name it. */
  attributeName?: (code: string) => string;
  /** The attribute in the record's revision, which formats its values. */
  attribute?: (code: string) => Attribute | undefined;
  /** Labels of the records that relationship changes link or unlink. */
  recordLabels?: ReadonlyMap<string, string>;
};

/** One audit event with the attribute changes it recorded. */
export const RecordChangeEvent = ({
  attribute,
  attributeName = (code) => code,
  changes,
  recordLabels,
}: Props) => {
  const { t } = useTranslation();
  const [event] = changes;
  const actorName = personName(event.actor_display_name, event.actor_email);
  const actor = actorName ? (
    <UserLabel
      avatarFileId={event.actor_avatar_file_id}
      name={actorName}
      userId={event.actor_user_id}
    />
  ) : event.executor_type === executorTypes.agent ? (
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
              userId={event.approved_by_user_id}
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
  ) : undefined;
  return (
    <Paper component="section" sx={{ mb: 2, p: 2 }}>
      {/* A user label is a block, so these lines are not paragraphs. */}
      <Typography component="div" sx={{ fontWeight: 'bold' }}>
        {actor} ·{' '}
        <Timestamp style="dateTimeSeconds" value={event.occurred_at} />
      </Typography>
      {approval && (
        <Typography color="text.secondary" component="div" variant="body2">
          {approval}
        </Typography>
      )}
      {changes.map((change, index) => (
        <Box
          // One save can add several relationships to the same attribute.
          key={`${change.attribute_id}-${change.context_id}-${change.change_kind}-${index}`}
          sx={{ mt: 1 }}
        >
          <Typography variant="body2">
            <strong>{attributeName(change.attribute_code)}</strong>
            {change.context_code ? ` (${change.context_code})` : ''}:{' '}
            {t(`records.changeKind.${change.change_kind}`)}
          </Typography>
          <Typography component="div" variant="body2">
            {relationshipChangeKinds.has(change.change_kind) ? (
              // A link names the record, not the ID the change stores.
              [change.before_value, change.after_value]
                .filter((value) => value !== null)
                .map((value) =>
                  typeof value === 'string'
                    ? (recordLabels?.get(value) ?? value)
                    : JSON.stringify(value),
                )
                .join(', ')
            ) : (
              <>
                <ChangeValue
                  attribute={attribute?.(change.attribute_code)}
                  value={change.before_value}
                />
                <Box component="span" sx={{ color: 'text.secondary', mx: 1 }}>
                  →
                </Box>
                <ChangeValue
                  attribute={attribute?.(change.attribute_code)}
                  value={change.after_value}
                />
              </>
            )}
          </Typography>
        </Box>
      ))}
    </Paper>
  );
};
