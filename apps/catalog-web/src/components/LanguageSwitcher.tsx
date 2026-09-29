import { FormControl, InputLabel, MenuItem, Select } from '@mui/material';
import { useId } from 'react';
import { useTranslation } from 'react-i18next';
import {
  defaultLanguage,
  type SupportedLanguage,
  supportedLanguages,
} from '../i18n';

const languageSwitcherMinWidth = 120;

const languageLabelKeys = {
  en: 'language.english',
  pl: 'language.polish',
} as const satisfies Record<SupportedLanguage, string>;

const isSupportedLanguage = (
  language: string | undefined,
): language is SupportedLanguage =>
  supportedLanguages.includes(language as SupportedLanguage);

export const LanguageSwitcher = () => {
  const { i18n, t } = useTranslation();
  const labelId = useId();
  const language = isSupportedLanguage(i18n.resolvedLanguage)
    ? i18n.resolvedLanguage
    : defaultLanguage;

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
