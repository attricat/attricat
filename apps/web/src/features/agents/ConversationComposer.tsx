import { useForm, useStore } from '@tanstack/react-form';
import { useState, type ReactNode, type Ref } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Alert, Box, Button } from '@mui/material';
import { RotateCcwIcon } from 'lucide-react';
import { uploadConversationFiles } from '../files/api';
import { sendMessage } from './api';
import {
  composerMinRows,
  composerSurfaces,
  type ComposerSurface,
} from './constants';
import { agentQueryKeys } from './queryKeys';
import { invalidateConversation } from './queryOptions';
import { MessageInputBox } from './MessageInputBox';
import { useMessageAttachments } from './useMessageAttachments';
import { errorMessage } from '../../api/errors';

/** The conversation messages go to, or how the first message starts one. */
type ComposerConversation =
  | { conversationId: string; createConversation?: never }
  | {
      /** `null` until the first message creates the conversation. */
      conversationId: null;
      /** Creates the conversation for the first message; returns its ID. */
      createConversation: (content: string) => Promise<string>;
    };

type ConversationComposerProps = ComposerConversation & {
  /** Focuses the message box when the composer starts a page of its own. */
  formRef?: Ref<HTMLFormElement>;
  minRows?: number;
  onSendingChange?: (isSending: boolean) => void;
  /**
   * Leads on to the conversation a message was sent to. It is skipped when
   * the composer has gone, such as after switching to another conversation
   * while sending; the conversation is refreshed either way.
   */
  onSent?: (conversationId: string) => void;
  sendDraft?: (
    content: string,
    conversationId: string,
    attachmentIds: string[],
  ) => Promise<unknown>;
  /** Holds messages back while an earlier run is unfinished. */
  sendBlocked?: boolean;
  /** Controls shown just above the message box. */
  settings?: ReactNode;
  /** The background the composer sits on, which its sticky footer repeats. */
  surface?: ComposerSurface;
  /** Keeps the composer at the bottom of a scrolling conversation. */
  sticky?: boolean;
};

export const ConversationComposer = (props: ConversationComposerProps) => {
  const {
    formRef,
    minRows = composerMinRows,
    onSendingChange,
    onSent,
    sendDraft,
    sendBlocked = false,
    settings,
    sticky = true,
    surface = composerSurfaces.page,
  } = props;
  const { t } = useTranslation();
  const client = useQueryClient();
  const form = useForm({ defaultValues: { content: '' } });
  const content = useStore(form.store, (state) => state.values.content);
  const attachments = useMessageAttachments();
  // Kept so a retry after a failed send does not create a second conversation.
  const [createdId, setCreatedId] = useState<string | null>(null);
  const conversationFor = async (content: string) => {
    if (props.conversationId !== null) return props.conversationId;
    if (createdId) return createdId;
    const id = await props.createConversation(content);
    setCreatedId(id);
    // Listed now, even if sending the first message fails.
    void client.invalidateQueries({
      queryKey: agentQueryKeys.conversations(),
    });
    return id;
  };
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
      const id = await conversationFor(content);
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
      void invalidateConversation(client, id);
    },
    onSettled: () => onSendingChange?.(false),
  });

  const submitMessage = () => {
    if (
      (content.trim() || attachments.files.length) &&
      !send.isPending &&
      !sendBlocked
    ) {
      // Passed per call so that it does not run once the composer is gone.
      send.mutate(
        {
          content,
          files: attachments.files,
          previousAttachmentIds: attachments.uploadedIds,
        },
        { onSuccess: (id) => onSent?.(id) },
      );
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
              bgcolor: surface,
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
      {settings}
      <MessageInputBox
        canSend={!sendBlocked}
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
