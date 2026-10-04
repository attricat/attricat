import {
  Autocomplete,
  FormControlLabel,
  Stack,
  Switch,
  TextField,
} from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { PublicationChannelChecks } from './api';
import { requiredRuleCodesFromInput } from './channelChecks';
import { MAX_REQUIRED_RULE_CODES } from './constants';

type Props = {
  disabled: boolean;
  requiredRuleCodes: readonly string[];
  requireValidEntity: boolean;
  ruleCodes: readonly string[];
  onChange: (checks: PublicationChannelChecks) => void;
};

/** Edits the checks an entity must pass before publication to a channel. */
export const ChannelChecksEditor = ({
  disabled,
  requiredRuleCodes,
  requireValidEntity,
  ruleCodes,
  onChange,
}: Props) => {
  const { t } = useTranslation();
  // Input beyond the API limit is not applied; say so instead of ignoring it.
  const [overLimit, setOverLimit] = useState(false);
  return (
    <Stack spacing={1}>
      <FormControlLabel
        control={
          <Switch
            checked={requireValidEntity}
            disabled={disabled}
            onChange={(_, checked) =>
              onChange({ require_valid_entity: checked })
            }
          />
        }
        label={t('exports.requireValidEntity')}
      />
      <Autocomplete
        disabled={disabled}
        freeSolo
        multiple
        onChange={(_, values) => {
          const codes = requiredRuleCodesFromInput(values);
          const exceeded = codes.length > MAX_REQUIRED_RULE_CODES;
          setOverLimit(exceeded);
          if (!exceeded) onChange({ required_rule_codes: codes });
        }}
        options={ruleCodes}
        renderInput={(params) => (
          <TextField
            {...params}
            error={overLimit}
            helperText={t(
              overLimit
                ? 'exports.requiredRulesLimit'
                : 'exports.requiredRulesHelp',
              { max: MAX_REQUIRED_RULE_CODES },
            )}
            label={t('exports.requiredRules')}
          />
        )}
        size="small"
        value={[...requiredRuleCodes]}
      />
    </Stack>
  );
};
