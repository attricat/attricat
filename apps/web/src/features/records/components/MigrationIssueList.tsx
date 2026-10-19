import {
  Alert,
  Box,
  Checkbox,
  FormControlLabel,
  Paper,
  Stack,
  Typography,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import {
  migrationIssueMessage,
  removalFieldCode,
  removedAttributeIssues,
  type MigrationIssue,
} from '../migrationReview';

type Props = {
  disabled: boolean;
  discardAttributes: readonly string[];
  issues: readonly MigrationIssue[];
  /** The attribute's name in the record's current revision. */
  attributeLabel: (attributeCode: string) => string;
  /** The attribute's stored value, if it has one to show. */
  previousValue: (attributeCode: string) => string;
  targetVersion: number;
  onDiscardChange: (attributeCode: string, discard: boolean) => void;
};

/**
 * Migration issues not tied to a target field: attributes the target revision
 * removes, each with its stored value and a confirmation to discard it, and
 * anything else as a warning.
 */
export const MigrationIssueList = ({
  disabled,
  discardAttributes,
  issues,
  attributeLabel,
  previousValue,
  targetVersion,
  onDiscardChange,
}: Props) => {
  const { t } = useTranslation();
  const removed = removedAttributeIssues(issues);
  const others = issues.filter(
    (issue) => !removed.some((removal) => removal === issue),
  );
  return (
    <>
      {others.map((issue) => (
        <Alert key={`${issue.attribute_code}:${issue.kind}`} severity="warning">
          {issue.attribute_code
            ? `${attributeLabel(issue.attribute_code)}: `
            : ''}
          {migrationIssueMessage(issue, t)}
        </Alert>
      ))}
      {removed.length > 0 && (
        <Box component="section">
          <Typography component="h2" sx={{ mb: 1 }} variant="h5">
            {t('records.removedInVersion', { version: targetVersion })}
          </Typography>
          <Typography color="text.secondary" sx={{ mb: 2 }} variant="body2">
            {t('records.removedInVersionDescription')}
          </Typography>
          <Stack spacing={1}>
            {removed.map((issue) => {
              const code = issue.attribute_code;
              const value = previousValue(code);
              return (
                <Paper
                  data-field-code={removalFieldCode(code)}
                  key={code}
                  sx={{ px: 2, py: 1.5 }}
                  variant="outlined"
                >
                  <Typography sx={{ fontWeight: 600 }}>
                    {attributeLabel(code)}
                  </Typography>
                  {value && (
                    <Typography
                      color="text.secondary"
                      sx={{ overflowWrap: 'anywhere' }}
                      variant="body2"
                    >
                      {t('records.previousValue', { value })}
                    </Typography>
                  )}
                  <FormControlLabel
                    control={
                      <Checkbox
                        checked={discardAttributes.includes(code)}
                        disabled={disabled}
                        onChange={(event) =>
                          onDiscardChange(code, event.target.checked)
                        }
                      />
                    }
                    label={t('records.discardStoredValue', {
                      attribute: attributeLabel(code),
                    })}
                  />
                </Paper>
              );
            })}
          </Stack>
        </Box>
      )}
    </>
  );
};
