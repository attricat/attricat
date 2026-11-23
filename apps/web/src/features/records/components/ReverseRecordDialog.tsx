import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { createRecord, type AttricatRecord } from '../api';
import {
  invalidateRecord,
  invalidateRecordSearches,
} from '../invalidateRecord';
import { ConfirmDialog } from '../../../components/ConfirmDialog';
import { attributeValueKinds } from '../valueTypes';

/**
 * Confirms reversing a movement record, then reports the reversal. The
 * server copies the original's quantity and links, and moves its amount back.
 */
export const ReverseRecordDialog = ({
  blueprintCode,
  recordId,
  reversalAttribute,
  onClose,
  onReversed,
}: {
  blueprintCode: string;
  recordId: string;
  /** The relationship through which a record reverses another. */
  reversalAttribute: string;
  onClose: () => void;
  onReversed: (reversal: AttricatRecord) => void;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const reversal = useMutation({
    mutationFn: () =>
      createRecord({
        blueprint: { code: blueprintCode },
        values: [
          {
            kind: attributeValueKinds.relationship,
            attribute_code: reversalAttribute,
            target_record_id: recordId,
          },
        ],
      }),
    onSuccess: (created) => {
      // The reversal changes a balance on another record.
      void invalidateRecordSearches(client);
      void invalidateRecord(client, recordId);
      onReversed(created);
    },
  });
  return (
    <ConfirmDialog
      confirmLabel={t('records.reverseRecord')}
      error={reversal.error}
      isPending={reversal.isPending}
      onClose={onClose}
      onConfirm={() => reversal.mutate()}
      title={t('records.reverseRecord')}
    >
      {t('records.reverseRecordConfirmation')}
    </ConfirmDialog>
  );
};
