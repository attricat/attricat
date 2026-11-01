import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { duplicateRecord, type AttricatRecord } from '../api';
import { invalidateRecordSearches } from '../invalidateRecord';
import { ConfirmDialog } from '../../../components/ConfirmDialog';
import { ApiRequestError } from '../../../api/request';
import { DUPLICATE_NEEDS_UNIQUE_VALUES_ERROR_CODE } from '../constants';
import { lexiconText } from '../../lexicon/lexicon';

/**
 * Confirms copying a record, then reports the copy. A record whose copy
 * would lack a required unique value cannot be copied, so the dialog then
 * offers creating another record of its blueprint instead.
 */
export const DuplicateRecordDialog = ({
  blueprint,
  recordId,
  onClose,
  onDuplicated,
}: {
  /** The record's blueprint, offered when the record cannot be copied. */
  blueprint?: { code: string; name: string };
  recordId: string;
  onClose: () => void;
  onDuplicated: (copy: AttricatRecord) => void;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const navigate = useNavigate();
  const duplication = useMutation({
    mutationFn: () => duplicateRecord(recordId),
    onSuccess: (copy) => {
      void invalidateRecordSearches(client);
      onDuplicated(copy);
    },
  });
  const createInstead =
    blueprint &&
    duplication.error instanceof ApiRequestError &&
    duplication.error.code === DUPLICATE_NEEDS_UNIQUE_VALUES_ERROR_CODE;

  return (
    <ConfirmDialog
      confirmLabel={
        createInstead
          ? t('records.createAnother', {
              blueprint: lexiconText(blueprint.name),
            })
          : t('records.duplicateRecord')
      }
      error={duplication.error}
      isPending={duplication.isPending}
      onClose={onClose}
      onConfirm={() => {
        if (!createInstead) {
          duplication.mutate();
          return;
        }
        onClose();
        void navigate({
          search: { blueprint: blueprint.code },
          to: '/records/new',
        });
      }}
      title={t('records.duplicateRecord')}
    >
      {t('records.duplicateRecordConfirmation')}
    </ConfirmDialog>
  );
};
