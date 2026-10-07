import { useQueryClient } from '@tanstack/react-query';
import { Button } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ApiErrorAlert } from '../../../components/CheckViolationsAlert';
import { invalidateEntity } from '../invalidateEntity';
import { ReusableAttributeAttachDialog } from './ReusableAttributeAttachDialog';
import { useReusableAttributeAttachment } from './useReusableAttributeAttachment';

/** Attaches reusable attributes or groups to an entity the user can edit. */
export const ReusableAttributeAttachControl = ({
  entityId,
  disabled = false,
}: {
  entityId: string;
  disabled?: boolean;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const [open, setOpen] = useState(false);
  // Remounts the dialog on every opening so its selections start empty.
  const [session, setSession] = useState(0);
  const reusable = useReusableAttributeAttachment(entityId, () => {
    setOpen(false);
    void invalidateEntity(client, entityId);
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
        variant="outlined"
      >
        {t('entities.addReusableAttributeOrGroup')}
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
