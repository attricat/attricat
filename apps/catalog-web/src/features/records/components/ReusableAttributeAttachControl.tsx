import { useQueryClient } from '@tanstack/react-query';
import { Button } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PlusIcon } from 'lucide-react';
import { ApiErrorAlert } from '../../../components/CheckViolationsAlert';
import { invalidateRecord } from '../invalidateRecord';
import { ReusableAttributeAttachDialog } from './ReusableAttributeAttachDialog';
import { useReusableAttributeAttachment } from './useReusableAttributeAttachment';

/** Attaches reusable attributes or groups to a record the user can edit. */
export const ReusableAttributeAttachControl = ({
  recordId,
  disabled = false,
}: {
  recordId: string;
  disabled?: boolean;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const [open, setOpen] = useState(false);
  // Remounts the dialog on every opening so its selections start empty.
  const [session, setSession] = useState(0);
  const reusable = useReusableAttributeAttachment(recordId, () => {
    setOpen(false);
    void invalidateRecord(client, recordId);
  });
  return (
    <>
      {reusable.error && (
        <ApiErrorAlert error={reusable.error} sx={{ mb: 2 }} />
      )}
      <Button
        disabled={disabled}
        onClick={() => {
          setSession((value) => value + 1);
          setOpen(true);
        }}
        startIcon={<PlusIcon />}
        variant="outlined"
      >
        {t('records.addReusableAttributeOrGroup')}
      </Button>
      <ReusableAttributeAttachDialog
        attachAttributeDisabled={
          reusable.attach.isPending || reusable.attributesUnavailable
        }
        attachGroupDisabled={
          reusable.attachGroup.isPending || reusable.groupsUnavailable
        }
        attributes={reusable.attributes}
        groups={reusable.groups}
        key={session}
        onAttachAttribute={(revisionId) => reusable.attach.mutate(revisionId)}
        onAttachGroup={(groupId) => reusable.attachGroup.mutate(groupId)}
        onClose={() => setOpen(false)}
        open={open}
      />
    </>
  );
};
