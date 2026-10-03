import {
  Autocomplete,
  FormControlLabel,
  Stack,
  Switch,
  TextField,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { PublicationChannelChecks } from './api';
import { MAX_REQUIRED_RULE_CODES } from './constants';

type Props = {
  disabled: boolean;
  requiredRuleCodes: readonly string[];
  requireValidEntity: boolean;
  ruleCodes: readonly string[];
  onChange: (checks: PublicationChannelChecks) => void;
};

/** Normalizes entered rule codes into the unique list the API accepts. */
export const requiredRuleCodesFromInput = (values: readonly string[]) => [
  ...new Set(values.map((value) => value.trim()).filter(Boolean)),
];

/** Edits the checks an entity must pass before publication to a channel. */
export const ChannelChecksEditor = ({
  disabled,
  requiredRuleCodes,
  requireValidEntity,
  ruleCodes,
  onChange,
}: Props) => {
  const { t } = useTranslation();
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
          if (codes.length <= MAX_REQUIRED_RULE_CODES)
            onChange({ required_rule_codes: codes });
        }}
        options={ruleCodes}
        renderInput={(params) => (
          <TextField
            {...params}
            helperText={t('exports.requiredRulesHelp', {
              max: MAX_REQUIRED_RULE_CODES,
            })}
            label={t('exports.requiredRules')}
          />
        )}
        size="small"
        value={[...requiredRuleCodes]}
      />
    </Stack>
  );
};
