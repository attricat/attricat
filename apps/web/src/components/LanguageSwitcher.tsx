import { useActiveLanguage } from '../i18n/useActiveLanguage';
import { FormControl, InputLabel, MenuItem, Select } from '@mui/material';
import { useId } from 'react';
import { useTranslation } from 'react-i18next';
import { type SupportedLanguage, supportedLanguages } from '../i18n';

const languageSwitcherMinWidth = 120;

const languageLabelKeys = {
  en: 'language.english',
  pl: 'language.polish',
} as const satisfies Record<SupportedLanguage, string>;

export const LanguageSwitcher = () => {
  const { i18n, t } = useTranslation();
  const labelId = useId();
  const language = useActiveLanguage();

  return (
    <FormControl size="small" sx={{ minWidth: languageSwitcherMinWidth }}>
      <InputLabel id={labelId}>{t('language.label')}</InputLabel>
      <Select
        label={t('language.label')}
        labelId={labelId}
        onChange={(event) => void i18n.changeLanguage(event.target.value)}
        value={language}
      >
        {supportedLanguages.map((item) => (
          <MenuItem key={item} value={item}>
            {t(languageLabelKeys[item])}
          </MenuItem>
        ))}
      </Select>
    </FormControl>
  );
};
