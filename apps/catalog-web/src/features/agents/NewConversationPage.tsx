import { useForm, useStore } from '@tanstack/react-form';
import { useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { useNavigate } from '@tanstack/react-router';
import { Alert, Box, Button } from '@mui/material';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { uploadConversationFiles } from '../files/api';
import { createConversation, sendMessage } from './api';
import {
  agentRoutes,
  newConversationMaxWidth,
  newConversationMinRows,
} from './constants';
import { conversationTitleFromFirstMessage } from './conversationTitle';
import { MessageInputBox } from './MessageInputBox';
import { agentQueryKeys } from './queryKeys';
import { useMessageAttachments } from './useMessageAttachments';

export const NewConversationPage = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const form = useForm({ defaultValues: { content: '' } });
  const content = useStore(form.store, (state) => state.values.content);
  const attachments = useMessageAttachments();
  const [conversationId, setConversationId] = useState<string | null>(null);
  const start = useMutation({
    meta: { toast: false },
    mutationFn: async ({
      content,
      files,
      previousAttachmentIds,
      existingConversationId,
    }: {
      content: string;
      files: File[];
      previousAttachmentIds: string[];
      existingConversationId: string | null;
    }) => {
      let id = existingConversationId;
      if (!id) {
        const conversation = await createConversation(
          conversationTitleFromFirstMessage(content),
        );
        id = conversation.id;
        setConversationId(id);
      }
      let attachmentIds = previousAttachmentIds;
      if (files.length > 0 && attachmentIds.length === 0) {
        const uploaded = await uploadConversationFiles(id, files);
        attachmentIds = uploaded.files.map((file) => file.id);
        attachments.setUploadedIds(attachmentIds);
      }
      await sendMessage(id, content, attachmentIds);
      return id;
    },
    onSuccess: async (id) => {
      await queryClient.invalidateQueries({ queryKey: agentQueryKeys.all() });
      await navigate({
        to: agentRoutes.detail,
        params: { conversationId: id },
      });
    },
  });
  const submit = () => {
    if ((content.trim() || attachments.files.length) && !start.isPending) {
      start.mutate({
        content,
        files: attachments.files,
        previousAttachmentIds: attachments.uploadedIds,
        existingConversationId: conversationId,
      });
    }
  };

  return (
    <PageContainer>
      <Box sx={{ maxWidth: newConversationMaxWidth, mx: 'auto' }}>
        <PageHeader
          description={t('agents.newConversationDescription')}
          title={t('agents.newAgentConversation')}
          titleVariant="h3"
        />
        <Box
          component="form"
          onSubmit={(event) => {
            event.preventDefault();
            submit();
          }}
          sx={{ mt: 4 }}
        >
          <MessageInputBox
            content={content}
            files={attachments.files}
            isPending={start.isPending}
            minRows={newConversationMinRows}
            onContentChange={(value) => form.setFieldValue('content', value)}
            onFilesChange={attachments.replaceFiles}
            onRemoveFile={attachments.removeFile}
            onSubmit={submit}
          />
        </Box>
        {start.isError && (
          <Alert
            action={
              <Button disabled={start.isPending} onClick={submit} size="small">
                {t('agents.retry')}
              </Button>
            }
            severity="error"
            sx={{ mt: 2 }}
          >
            {t('agents.messageNotSent')}: {start.error.message}
          </Alert>
        )}
      </Box>
    </PageContainer>
  );
};
