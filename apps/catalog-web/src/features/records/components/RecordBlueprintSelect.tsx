import { useQuery } from '@tanstack/react-query';
import { Alert, MenuItem, TextField, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { listRecordBlueprints } from '../api';
import { recordQueryKeys } from '../queryKeys';
import { lexiconText } from '../../lexicon/lexicon';

type Props = {
  onChange: (blueprintCode: string) => void;
  value: string;
};

/** Lets the user pick a blueprint before a record form can be shown. */
export const RecordBlueprintSelect = ({ onChange, value }: Props) => {
  const { t } = useTranslation();
  const blueprints = useQuery({
    queryKey: recordQueryKeys.blueprints(),
    queryFn: ({ signal }) => listRecordBlueprints(signal),
  });
  return (
    <>
      <Typography variant="h6">{t('records.chooseBlueprint')}</Typography>
      <TextField
        select
        required
        label={t('records.blueprint')}
        onChange={(event) => onChange(event.target.value)}
        value={value}
      >
        <MenuItem value="">{t('records.selectBlueprint')}</MenuItem>
        {(blueprints.data ?? []).map((option) => (
          <MenuItem key={option.code} value={option.code}>
            {t('records.blueprintOption', {
              name: lexiconText(option.name),
              code: option.code,
            })}
          </MenuItem>
        ))}
      </TextField>
      {blueprints.isError && (
        <Alert severity="error">{t('records.couldNotLoadBlueprints')}</Alert>
      )}
    </>
  );
};
