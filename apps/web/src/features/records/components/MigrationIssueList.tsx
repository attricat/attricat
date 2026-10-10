import { Alert, Checkbox, FormControlLabel, FormGroup } from '@mui/material';
import { useTranslation } from 'react-i18next';
import {
  removedAttributeIssues,
  type MigrationIssue,
} from '../migrationReview';

type Props = {
  disabled: boolean;
  discardAttributes: readonly string[];
  issues: readonly MigrationIssue[];
  onDiscardChange: (attributeCode: string, discard: boolean) => void;
};

/** Migration issues not tied to a target field, with removal confirmations. */
export const MigrationIssueList = ({
  disabled,
  discardAttributes,
  issues,
  onDiscardChange,
}: Props) => {
  const { t } = useTranslation();
  return (
    <>
      {issues.map((issue) => (
        <Alert
          key={`${issue.attribute_code}:${issue.message}`}
          severity="warning"
          sx={{ mt: 2 }}
        >
          {issue.attribute_code ? `${issue.attribute_code}: ` : ''}
          {issue.message}
        </Alert>
      ))}
      <FormGroup sx={{ mt: 2 }}>
        {removedAttributeIssues(issues).map((issue) => (
          <FormControlLabel
            control={
              <Checkbox
                checked={discardAttributes.includes(issue.attribute_code)}
                disabled={disabled}
                onChange={(event) =>
                  onDiscardChange(issue.attribute_code, event.target.checked)
                }
              />
            }
            key={issue.attribute_code}
            label={t('records.confirmRemoval', {
              attribute: issue.attribute_code,
            })}
          />
        ))}
      </FormGroup>
    </>
  );
};
