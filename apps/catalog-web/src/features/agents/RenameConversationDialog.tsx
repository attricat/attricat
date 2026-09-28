import { useRef } from 'react';
import { useForm } from '@tanstack/react-form';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  Alert,
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  TextField,
} from '@mui/material';
import { updateConversationTitle } from './api';
import { agentQueryKeys } from './queryKeys';

const maximumTitleBytes = 512;

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
  const inputRef = useRef<HTMLInputElement>(null);
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
    <Dialog
      fullWidth
      maxWidth="sm"
      onClose={() => {
        if (!rename.isPending) onClose();
      }}
      open
      slotProps={{ transition: { onEntered: () => inputRef.current?.focus() } }}
    >
      <Box
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          void form.handleSubmit();
        }}
      >
        <DialogTitle>{t('agents.renameConversation')}</DialogTitle>
        <DialogContent>
          <form.Field
            name="title"
            validators={{
              onChange: ({ value }) => validateTitle(value),
              onSubmit: ({ value }) => validateTitle(value),
            }}
          >
            {(field) => (
              <TextField
                autoComplete="off"
                error={field.state.meta.errors.length > 0}
                fullWidth
                helperText={field.state.meta.errors[0]}
                inputRef={inputRef}
                label={t('agents.conversationTitle')}
                margin="dense"
                onBlur={field.handleBlur}
                onChange={(event) => field.handleChange(event.target.value)}
                slotProps={{ htmlInput: { maxLength: maximumTitleBytes } }}
                value={field.state.value}
              />
            )}
          </form.Field>
          {rename.error && (
            <Alert severity="error" sx={{ mt: 2 }}>
              {rename.error.message}
            </Alert>
          )}
        </DialogContent>
        <DialogActions>
          <Button disabled={rename.isPending} onClick={onClose}>
            {t('common.cancel')}
          </Button>
          <Button disabled={rename.isPending} type="submit" variant="contained">
            {t('agents.saveTitle')}
          </Button>
        </DialogActions>
      </Box>
    </Dialog>
  );
};
