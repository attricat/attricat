import { useForm } from '@tanstack/react-form';
import { useNavigate } from '@tanstack/react-router';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Alert,
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { createConversation, sendMessage } from '../agents/api';
import { agentQueryKeys } from '../agents/queryKeys';
import type { RecordItem } from '../records/api';
import { displayLabel } from '../records/recordDisplay';
import { selectedRecordsMessage } from './agentSelection';
import {
  agentRecordListMaxHeight,
  agentInstructionsRows,
  maximumAgentConversationBlueprintNameLength,
  maximumAgentInstructionsLength,
} from './constants';

export const SendSelectedToAgentDialog = ({
  blueprintName,
  records,
  onClose,
  onSuccess,
}: {
  blueprintName: string;
  records: RecordItem[];
  onClose: () => void;
  onSuccess: () => void;
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [conversationId, setConversationId] = useState<string | null>(null);
  const start = useMutation({
    meta: { toast: false },
    mutationFn: async (instructions: string) => {
      let id = conversationId;
      if (!id) {
        const conversation = await createConversation(
          t('explorer.agentConversationTitle', {
            count: records.length,
            blueprint: blueprintName.slice(
              0,
              maximumAgentConversationBlueprintNameLength,
            ),
          }),
        );
        id = conversation.id;
        setConversationId(id);
      }
      await sendMessage(
        id,
        selectedRecordsMessage(t, instructions, blueprintName, records),
      );
      return id;
    },
    onSuccess: async (id) => {
      await queryClient.invalidateQueries({ queryKey: agentQueryKeys.all() });
      onSuccess();
      await navigate({
        to: '/agents/$conversationId',
        params: { conversationId: id },
      });
    },
  });
  const form = useForm({
    defaultValues: { instructions: t('explorer.agentDefaultInstructions') },
    onSubmit: ({ value }) => {
      start.mutate(value.instructions);
    },
  });

  return (
    <Dialog
      fullWidth
      maxWidth="sm"
      onClose={start.isPending ? undefined : onClose}
      open
    >
      <DialogTitle>{t('explorer.sendToAgentConversation')}</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ pt: 1 }}>
          <Typography>
            {t('explorer.selectedCountForBlueprint', {
              blueprint: blueprintName,
              count: records.length,
            })}
          </Typography>
          <Box
            component="ul"
            sx={{
              maxHeight: agentRecordListMaxHeight,
              my: 0,
              overflowY: 'auto',
              pl: 3,
            }}
          >
            {records.map((record) => (
              <li key={record.id}>
                <Typography variant="body2">
                  {t('explorer.agentRecordListItem', {
                    recordId: record.id,
                    label: displayLabel(record.display, record.id),
                  })}
                </Typography>
              </li>
            ))}
          </Box>
          <form.Field name="instructions">
            {(field) => (
              <TextField
                required
                disabled={start.isPending}
                fullWidth
                label={t('explorer.agentInstructions')}
                multiline
                slotProps={{
                  htmlInput: { maxLength: maximumAgentInstructionsLength },
                }}
                onBlur={field.handleBlur}
                onChange={(event) => field.handleChange(event.target.value)}
                rows={agentInstructionsRows}
                value={field.state.value}
              />
            )}
          </form.Field>
          {start.isError && (
            <Alert severity="error">{start.error.message}</Alert>
          )}
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button disabled={start.isPending} onClick={onClose}>
          {t('explorer.cancel')}
        </Button>
        <form.Subscribe selector={(state) => state.values.instructions}>
          {(instructions) => (
            <Button
              disabled={start.isPending || !instructions.trim()}
              onClick={() => void form.handleSubmit()}
              variant="contained"
            >
              {t('explorer.sendToAgentConversation')}
            </Button>
          )}
        </form.Subscribe>
      </DialogActions>
    </Dialog>
  );
};
