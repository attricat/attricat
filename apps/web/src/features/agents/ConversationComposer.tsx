import { useForm, useStore } from '@tanstack/react-form';
import { useState, type Ref } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation } from '@tanstack/react-query';
import { Alert, Box, Button } from '@mui/material';
import { RotateCcwIcon } from 'lucide-react';
import { uploadConversationFiles } from '../files/api';
import { sendMessage } from './api';
import { composerMinRows } from './constants';
import { MessageInputBox } from './MessageInputBox';
import { useMessageAttachments } from './useMessageAttachments';
import { errorMessage } from '../../api/errors';

type ConversationComposerProps = {
  /** `null` until the first message creates the conversation. */
  conversationId: string | null;
  /** Creates the conversation for the first message; returns its ID. */
  createConversation?: (content: string) => Promise<string>;
  /** Focuses the message box when the composer starts a page of its own. */
  formRef?: Ref<HTMLFormElement>;
  minRows?: number;
  onSendingChange?: (isSending: boolean) => void;
  onSent: (conversationId: string) => void;
  sendDraft?: (
    content: string,
    conversationId: string,
    attachmentIds: string[],
  ) => Promise<unknown>;
  /** Keeps the composer at the bottom of a scrolling conversation. */
  sticky?: boolean;
};

export const ConversationComposer = ({
  conversationId,
  createConversation,
  formRef,
  minRows = composerMinRows,
  onSendingChange,
  onSent,
  sendDraft,
  sticky = true,
}: ConversationComposerProps) => {
  const { t } = useTranslation();
  const form = useForm({ defaultValues: { content: '' } });
  const content = useStore(form.store, (state) => state.values.content);
  const attachments = useMessageAttachments();
  // Kept so a retry after a failed send does not create a second conversation.
  const [createdId, setCreatedId] = useState<string | null>(null);
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
      let id = conversationId ?? createdId;
      if (!id) {
        if (!createConversation) {
          throw new Error('A conversation is required to send a message');
        }
        id = await createConversation(content);
        setCreatedId(id);
      }
      let attachmentIds = previousAttachmentIds;
      if (files.length > 0 && attachmentIds.length === 0) {
        const uploaded = await uploadConversationFiles(id, files);
        attachmentIds = uploaded.files.map((file) => file.id);
        attachments.setUploadedIds(attachmentIds);
      }
      await (sendDraft
        ? sendDraft(content, id, attachmentIds)
        : sendMessage(id, content, attachmentIds));
      return id;
    },
    onMutate: () => onSendingChange?.(true),
    onSuccess: (id) => {
      form.reset();
      attachments.reset();
      onSent(id);
    },
    onSettled: () => onSendingChange?.(false),
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
      ref={formRef}
      sx={
        sticky
          ? {
              bgcolor: 'background.default',
              bottom: 0,
              mx: 'auto',
              pb: { xs: 1, md: 2 },
              position: 'sticky',
              pt: 2,
              width: '100%',
            }
          : { mt: 4 }
      }
    >
      <MessageInputBox
        content={content}
        files={attachments.files}
        isPending={send.isPending}
        minRows={minRows}
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
          {t('agents.messageNotSent')}: {errorMessage(send.error)}
        </Alert>
      )}
    </Box>
  );
};
