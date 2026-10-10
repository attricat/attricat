import { useState } from 'react';
import {
  Box,
  Button,
  Checkbox,
  FormControlLabel,
  Paper,
  Stack,
  Typography,
} from '@mui/material';
import { useTranslation } from 'react-i18next';

type Proposal = {
  fields: Record<string, string>;
  explanation: string;
  baseValues?: Record<string, string | null>;
};

export const DraftProposal = ({
  proposal,
  onApply,
  getDraftValues,
}: {
  proposal: Proposal;
  onApply: (fields: Record<string, string>) => void;
  getDraftValues?: () => Record<string, string>;
}) => {
  const { t } = useTranslation();
  const [selected, setSelected] = useState<string[]>(
    Object.keys(proposal.fields),
  );
  const [conflicts, setConflicts] = useState<string[]>([]);
  return (
    <Paper variant="outlined" sx={{ mt: 1, p: 2 }}>
      <Typography sx={{ mb: 1 }}>{proposal.explanation}</Typography>
      <Stack>
        {Object.entries(proposal.fields).map(([code, value]) => (
          <FormControlLabel
            key={code}
            control={
              <Checkbox
                checked={selected.includes(code)}
                onChange={(event) =>
                  setSelected((current) =>
                    event.target.checked
                      ? [...current, code]
                      : current.filter((item) => item !== code),
                  )
                }
              />
            }
            label={
              <Box sx={{ overflowWrap: 'anywhere' }}>
                <strong>{code}</strong>: {value}
              </Box>
            }
          />
        ))}
      </Stack>
      <Button
        disabled={!selected.length}
        onClick={() => {
          const current = getDraftValues?.() ?? {};
          const changed = selected.filter(
            (code) =>
              proposal.baseValues &&
              current[code] !== (proposal.baseValues[code] ?? undefined),
          );
          setConflicts(changed);
          onApply(
            Object.fromEntries(
              Object.entries(proposal.fields).filter(
                ([code]) => selected.includes(code) && !changed.includes(code),
              ),
            ),
          );
        }}
        variant="outlined"
      >
        {t('records.applySmartFill')}
      </Button>
      {conflicts.length > 0 && (
        <Typography color="text.secondary" variant="body2">
          {t('records.draftChanged', { fields: conflicts.join(', ') })}
        </Typography>
      )}
    </Paper>
  );
};
