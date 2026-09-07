import { FormControl, InputLabel, MenuItem, Select } from '@mui/material';
import { useTranslation } from 'react-i18next';
import i18n from '../i18n';

export const LanguageSwitcher = () => {
  const { i18n: translationI18n, t } = useTranslation();
  const language = translationI18n.resolvedLanguage === 'pl' ? 'pl' : 'en';

  return (
    <FormControl size="small" sx={{ minWidth: 120 }}>
      <InputLabel id="language-select-label">{t('language.label')}</InputLabel>
      <Select
        label={t('language.label')}
        labelId="language-select-label"
        onChange={(event) => void i18n.changeLanguage(event.target.value)}
        value={language}
      >
        <MenuItem value="en">{t('language.english')}</MenuItem>
        <MenuItem value="pl">{t('language.polish')}</MenuItem>
      </Select>
    </FormControl>
  );
};
