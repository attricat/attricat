import { useForm, useStore } from '@tanstack/react-form';
import { useTranslation } from 'react-i18next';
import { useMutation } from '@tanstack/react-query';
import { Alert, Box, Button } from '@mui/material';
import { RotateCcwIcon } from 'lucide-react';
import { uploadConversationFiles } from '../files/api';
import { sendMessage } from './api';
import { composerMinRows } from './constants';
import { MessageInputBox } from './MessageInputBox';
import { useMessageAttachments } from './useMessageAttachments';

export type ConversationComposerProps = {
  conversationId: string;
  onSendingChange: (isSending: boolean) => void;
  onSent: () => void;
  sendDraft?: (
    content: string,
    conversationId: string,
    attachmentIds: string[],
  ) => Promise<unknown>;
};

export const ConversationComposer = ({
  conversationId,
  onSendingChange,
  onSent,
  sendDraft,
}: ConversationComposerProps) => {
  const { t } = useTranslation();
  const form = useForm({ defaultValues: { content: '' } });
  const content = useStore(form.store, (state) => state.values.content);
  const attachments = useMessageAttachments();
  const send = useMutation({
    meta: { toast: false },
    mutationFn: async ({
      content,
      files,
      previousAttachmentIds,
    }: {
      content: string;
      files: File[];
      previousAttachmentIds: string[];
    }) => {
      let attachmentIds = previousAttachmentIds;
      if (files.length > 0 && attachmentIds.length === 0) {
        const uploaded = await uploadConversationFiles(conversationId, files);
        attachmentIds = uploaded.files.map((file) => file.id);
        attachments.setUploadedIds(attachmentIds);
      }
      return sendDraft
        ? sendDraft(content, conversationId, attachmentIds)
        : sendMessage(conversationId, content, attachmentIds);
    },
    onMutate: () => onSendingChange(true),
    onSuccess: () => {
      form.reset();
      attachments.reset();
      onSent();
    },
    onSettled: () => onSendingChange(false),
  });

  const submitMessage = () => {
    if ((content.trim() || attachments.files.length) && !send.isPending) {
      send.mutate({
        content,
        files: attachments.files,
        previousAttachmentIds: attachments.uploadedIds,
      });
    }
  };

  return (
    <Box
      component="form"
      onSubmit={(event) => {
        event.preventDefault();
        submitMessage();
      }}
      sx={{
        bgcolor: 'background.default',
        bottom: 0,
        mx: 'auto',
        pb: { xs: 1, md: 2 },
        position: 'sticky',
        pt: 2,
        width: '100%',
      }}
    >
      <MessageInputBox
        content={content}
        files={attachments.files}
        isPending={send.isPending}
        minRows={composerMinRows}
        onContentChange={(value) => form.setFieldValue('content', value)}
        onFilesChange={attachments.replaceFiles}
        onRemoveFile={attachments.removeFile}
        onSubmit={submitMessage}
      />
      {send.isError && (
        <Alert
          action={
            <Button
              disabled={send.isPending}
              onClick={submitMessage}
              size="small"
              startIcon={<RotateCcwIcon />}
            >
              {t('agents.retry')}
            </Button>
          }
          severity="error"
          sx={{ mt: 1 }}
        >
          {t('agents.messageNotSent')}: {send.error.message}
        </Alert>
      )}
    </Box>
  );
};
