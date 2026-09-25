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
import { agentQueryKeys } from '../agents/query-keys';
import type { EntityItem } from '../entities/api';
import { displayLabel } from '../entities/entity-display';
import { selectedEntitiesMessage } from './agent-selection';

export const SendSelectedToAgentDialog = ({
  blueprintName,
  entities,
  onClose,
  onSuccess,
}: {
  blueprintName: string;
  entities: EntityItem[];
  onClose: () => void;
  onSuccess: () => void;
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [conversationId, setConversationId] = useState<string | null>(null);
  const form = useForm({
    defaultValues: { instructions: t('explorer.agentDefaultInstructions') },
    onSubmit: ({ value }) => {
      start.mutate(value.instructions);
    },
  });
  const start = useMutation({
    meta: { toast: false },
    mutationFn: async (instructions: string) => {
      let id = conversationId;
      if (!id) {
        const conversation = await createConversation(
          t('explorer.agentConversationTitle', {
            count: entities.length,
            blueprint: blueprintName.slice(0, 120),
          }),
        );
        id = conversation.id;
        setConversationId(id);
      }
      await sendMessage(
        id,
        selectedEntitiesMessage(instructions, blueprintName, entities),
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
            {t('explorer.selectedCount', { count: entities.length })} ·{' '}
            {blueprintName}
          </Typography>
          <Box
            component="ul"
            sx={{ maxHeight: 140, my: 0, overflowY: 'auto', pl: 3 }}
          >
            {entities.map((entity) => (
              <li key={entity.id}>
                <Typography variant="body2">
                  {displayLabel(entity.display, entity.id)} · {entity.id}
                </Typography>
              </li>
            ))}
          </Box>
          <form.Field name="instructions">
            {(field) => (
              <TextField
                disabled={start.isPending}
                fullWidth
                label={t('explorer.agentInstructions')}
                multiline
                slotProps={{ htmlInput: { maxLength: 1000 } }}
                onBlur={field.handleBlur}
                onChange={(event) => field.handleChange(event.target.value)}
                rows={3}
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
          {t('explorer.cancelAttributeFilter')}
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
