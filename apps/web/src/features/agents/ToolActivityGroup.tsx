import {
  Accordion,
  AccordionDetails,
  AccordionSummary,
  Stack,
  Typography,
} from '@mui/material';
import { ChevronDownIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { ConversationMessage } from './schemas';
import { callName, requestedCalls } from './toolActivity';
import { TranscriptMessage } from './TranscriptMessage';

/**
 * Consecutive tool calls and results, collapsed to one line so a run of
 * lookups does not push the agent's reply out of view.
 */
export const ToolActivityGroup = ({
  messages,
}: {
  messages: ConversationMessage[];
}) => {
  const { t } = useTranslation();
  const calls = messages.flatMap(requestedCalls);
  const names = [
    ...new Set(calls.map(callName).filter((name) => name !== null)),
  ];
  return (
    <Accordion
      disableGutters
      elevation={0}
      slotProps={{ transition: { unmountOnExit: true } }}
      sx={{ bgcolor: 'transparent' }}
      variant="outlined"
    >
      <AccordionSummary expandIcon={<ChevronDownIcon />}>
        <Stack sx={{ minWidth: 0 }}>
          <Typography variant="body2">
            {t('agents.toolActivity', { count: calls.length })}
          </Typography>
          {names.length > 0 && (
            <Typography
              color="text.secondary"
              noWrap
              sx={{ fontFamily: 'monospace' }}
              variant="caption"
            >
              {names.join(', ')}
            </Typography>
          )}
        </Stack>
      </AccordionSummary>
      <AccordionDetails>
        <Stack spacing={2}>
          {messages.map((message) => (
            <TranscriptMessage key={message.id} message={message} />
          ))}
        </Stack>
      </AccordionDetails>
    </Accordion>
  );
};
