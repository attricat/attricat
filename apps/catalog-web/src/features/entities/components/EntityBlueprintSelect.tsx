import { useQuery } from '@tanstack/react-query';
import { Alert, MenuItem, TextField, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { listEntityBlueprints } from '../api';
import { entityQueryKeys } from '../queryKeys';

type Props = {
  onChange: (blueprintCode: string) => void;
  value: string;
};

/** Lets the user pick a blueprint before an entity form can be shown. */
export const EntityBlueprintSelect = ({ onChange, value }: Props) => {
  const { t } = useTranslation();
  const blueprints = useQuery({
    queryKey: entityQueryKeys.blueprints(),
    queryFn: ({ signal }) => listEntityBlueprints(signal),
  });
  return (
    <>
      <Typography variant="h6">{t('entities.chooseBlueprint')}</Typography>
      <TextField
        select
        required
        label={t('entities.blueprint')}
        onChange={(event) => onChange(event.target.value)}
        value={value}
      >
        <MenuItem value="">{t('entities.selectBlueprint')}</MenuItem>
        {(blueprints.data ?? []).map((option) => (
          <MenuItem key={option.code} value={option.code}>
            {t('entities.blueprintOption', {
              name: option.name,
              code: option.code,
            })}
          </MenuItem>
        ))}
      </TextField>
      {blueprints.isError && (
        <Alert severity="error">{t('entities.couldNotLoadBlueprints')}</Alert>
      )}
    </>
  );
};
