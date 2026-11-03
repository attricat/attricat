import {
  Box,
  Checkbox,
  FormControlLabel,
  Paper,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import type {
  MigrationContextAnswer,
  MigrationContextValue,
} from '../migrationReview';

type Props = {
  answers: Readonly<Record<string, MigrationContextAnswer>>;
  attributeLabel: (attributeCode: string) => string;
  contextLabel: (contextId: string) => string;
  disabled: boolean;
  onAnswer: (key: string, answer: MigrationContextAnswer) => void;
  /** The value stored in that context, to show for reference. */
  previousValue: (attributeCode: string, contextId: string) => string;
  values: readonly MigrationContextValue[];
};

/**
 * Stored values outside the default context that the new revision can't take.
 * The form's fields edit the default context, so each is answered here: with a
 * replacement in its own context, or by removing it.
 */
export const MigrationContextValues = ({
  answers,
  attributeLabel,
  contextLabel,
  disabled,
  onAnswer,
  previousValue,
  values,
}: Props) => {
  const { t } = useTranslation();
  if (values.length === 0) return null;
  return (
    <Box component="section">
      <Typography component="h2" sx={{ mb: 1 }} variant="h5">
        {t('records.migrationOtherContexts')}
      </Typography>
      <Typography color="text.secondary" sx={{ mb: 2 }} variant="body2">
        {t('records.migrationOtherContextsDescription')}
      </Typography>
      <Stack spacing={1}>
        {values.map((value) => {
          const answer = answers[value.key] ?? { remove: false, text: '' };
          const stored = previousValue(value.attributeCode, value.contextId);
          const name = attributeLabel(value.attributeCode);
          const context = contextLabel(value.contextId);
          return (
            <Paper
              data-field-code={value.key}
              key={value.key}
              sx={{ px: 2, py: 1.5 }}
              variant="outlined"
            >
              <Typography sx={{ fontWeight: 'fontWeightBold' }}>
                {t('records.migrationContextValue', {
                  attribute: name,
                  context,
                })}
              </Typography>
              <Typography color="text.secondary" variant="body2">
                {value.message}
              </Typography>
              {stored && (
                <Typography
                  color="text.secondary"
                  sx={{ overflowWrap: 'anywhere' }}
                  variant="body2"
                >
                  {t('records.previousValue', { value: stored })}
                </Typography>
              )}
              {value.replaceable && (
                <TextField
                  disabled={disabled || answer.remove}
                  fullWidth
                  label={t('records.migrationReplacement', { context })}
                  onChange={(event) =>
                    onAnswer(value.key, {
                      ...answer,
                      text: event.target.value,
                    })
                  }
                  size="small"
                  sx={{ mt: 1.5 }}
                  value={answer.text}
                />
              )}
              <FormControlLabel
                control={
                  <Checkbox
                    checked={answer.remove}
                    disabled={disabled}
                    onChange={(event) =>
                      onAnswer(value.key, {
                        ...answer,
                        remove: event.target.checked,
                      })
                    }
                  />
                }
                label={t('records.migrationRemoveContextValue', { context })}
              />
            </Paper>
          );
        })}
      </Stack>
    </Box>
  );
};
