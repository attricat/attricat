import { useForm } from '@tanstack/react-form';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { TextField } from '@mui/material';
import { maximumTitleBytes } from './constants';
import { updateConversationTitle } from './api';
import { agentQueryKeys } from './queryKeys';
import { FormDialog } from '../../components/FormDialog';

export const RenameConversationDialog = ({
  conversationId,
  initialTitle,
  onClose,
}: {
  conversationId: string;
  initialTitle: string;
  onClose: () => void;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const rename = useMutation({
    mutationFn: (title: string) =>
      updateConversationTitle(conversationId, title),
    onSuccess: () => {
      void client.invalidateQueries({
        queryKey: agentQueryKeys.conversations(),
      });
      onClose();
    },
  });
  const validateTitle = (value: string) => {
    if (!value.trim()) return t('agents.titleRequired');
    if (new Blob([value.trim()]).size > maximumTitleBytes)
      return t('agents.titleTooLong');
    return undefined;
  };
  const form = useForm({
    defaultValues: { title: initialTitle },
    onSubmit: ({ value }) => rename.mutate(value.title.trim()),
  });
  return (
    <FormDialog
      error={rename.error}
      isPending={rename.isPending}
      onClose={onClose}
      onSubmit={() => void form.handleSubmit()}
      submitLabel={t('agents.saveTitle')}
      title={t('agents.renameConversation')}
    >
      <form.Field
        name="title"
        validators={{
          onChange: ({ value }) => validateTitle(value),
          onSubmit: ({ value }) => validateTitle(value),
        }}
      >
        {(field) => (
          <TextField
            required
            autoComplete="off"
            error={field.state.meta.errors.length > 0}
            fullWidth
            helperText={field.state.meta.errors[0]}
            label={t('agents.conversationTitle')}
            margin="dense"
            onBlur={field.handleBlur}
            onChange={(event) => field.handleChange(event.target.value)}
            slotProps={{ htmlInput: { maxLength: maximumTitleBytes } }}
            value={field.state.value}
          />
        )}
      </form.Field>
    </FormDialog>
  );
};
