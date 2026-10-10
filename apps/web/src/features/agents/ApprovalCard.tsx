import {
  Accordion,
  AccordionDetails,
  AccordionSummary,
  Box,
  Button,
  Paper,
  Stack,
  Typography,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import { CheckIcon, XIcon } from 'lucide-react';
import { jsonIndent } from './constants';
import type { AgentToolCall } from './schemas';

export const ApprovalCard = ({
  call,
  isDeciding,
  onDecide,
}: {
  call: AgentToolCall;
  isDeciding: boolean;
  onDecide: (approvalId: string, approved: boolean) => void;
}) => {
  const { t } = useTranslation();
  return (
    <Paper sx={{ border: 1, borderColor: 'warning.main', p: 2.5 }}>
      <Typography variant="h6">
        {t('agents.approvalNeeded', { tool: call.tool_name })}
      </Typography>
      {call.change_summary && (
        <Typography sx={{ mt: 1 }}>{call.change_summary}</Typography>
      )}
      <Accordion
        disableGutters
        elevation={0}
        sx={{ bgcolor: 'action.hover', mt: 1.5 }}
      >
        <AccordionSummary>{t('agents.showProposedInput')}</AccordionSummary>
        <AccordionDetails>
          <Box
            component="pre"
            sx={{ m: 0, overflow: 'auto', whiteSpace: 'pre-wrap' }}
          >
            {JSON.stringify(call.arguments, null, jsonIndent)}
          </Box>
        </AccordionDetails>
      </Accordion>
      <Stack direction="row" spacing={1} sx={{ mt: 2 }}>
        <Button
          color="success"
          disabled={isDeciding}
          onClick={() => onDecide(call.id, true)}
          startIcon={<CheckIcon />}
          variant="contained"
        >
          {t('agents.approve')}
        </Button>
        <Button
          color="error"
          disabled={isDeciding}
          onClick={() => onDecide(call.id, false)}
          startIcon={<XIcon />}
          variant="outlined"
        >
          {t('agents.reject')}
        </Button>
      </Stack>
    </Paper>
  );
};
