import { useTranslation } from 'react-i18next';
import {
  Box,
  Button,
  List,
  ListItem,
  ListItemText,
  Paper,
  Stack,
  Typography,
} from '@mui/material';
import {
  DEFAULT_PLURAL_CATEGORY,
  LEXICON_MANAGEMENT_NAMESPACES,
} from './constants';
import type { LexiconEntryDraft } from './entries';
import type { LexiconReport } from './schemas';

type LanguageCoverage = LexiconReport['languages'][number];

/** Gaps from the coverage report, each with a shortcut to fill it. */
export const LexiconCoverageSection = ({
  canEdit,
  coverage,
  languageName,
  onTranslate,
}: {
  canEdit: boolean;
  coverage: LanguageCoverage;
  languageName: string;
  onTranslate: (draft: LexiconEntryDraft) => void;
}) => {
  const { t } = useTranslation(LEXICON_MANAGEMENT_NAMESPACES);
  const gapCount =
    coverage.untranslated.length + coverage.missing_plural_categories.length;
  const draft = (
    key: string,
    context: string | null,
    category = DEFAULT_PLURAL_CATEGORY,
  ): LexiconEntryDraft => ({
    key,
    context: context ?? '',
    plural_category: category,
    text: '',
  });
  const referenceLabel = (key: string, context: string | null) =>
    context ? t('lexicon.keyWithContext', { key, context }) : key;
  return (
    <Paper component="section" sx={{ mt: 3 }}>
      <Box sx={{ borderBottom: 1, borderColor: 'divider', p: 2 }}>
        <Typography component="h2" variant="h6">
          {gapCount
            ? t('lexicon.needsTranslation', { count: gapCount })
            : t('lexicon.allTranslated')}
        </Typography>
        <Typography color="text.secondary" variant="body2">
          {gapCount
            ? t('lexicon.needsTranslationDescription', {
                language: languageName,
              })
            : t('lexicon.fullyTranslated', { language: languageName })}
        </Typography>
      </Box>
      {gapCount > 0 && (
        <List dense disablePadding>
          {coverage.untranslated.map((reference) => (
            <ListItem
              divider
              key={`untranslated:${reference.key}:${reference.context ?? ''}`}
              secondaryAction={
                canEdit && (
                  <Button
                    onClick={() =>
                      onTranslate(draft(reference.key, reference.context))
                    }
                    size="small"
                  >
                    {t('lexicon.translate')}
                  </Button>
                )
              }
            >
              <ListItemText
                primary={referenceLabel(reference.key, reference.context)}
                secondary={t('lexicon.untranslated')}
              />
            </ListItem>
          ))}
          {coverage.missing_plural_categories.map((gap) => (
            <ListItem
              divider
              key={`plural:${gap.key}:${gap.context ?? ''}`}
              sx={{ flexWrap: 'wrap', gap: 1 }}
            >
              <ListItemText
                primary={referenceLabel(gap.key, gap.context)}
                secondary={t('lexicon.missingPluralCategories', {
                  categories: gap.missing.join(', '),
                })}
              />
              {canEdit && (
                <Stack direction="row" spacing={1}>
                  {gap.missing.map((category) => (
                    <Button
                      key={category}
                      onClick={() =>
                        onTranslate(draft(gap.key, gap.context, category))
                      }
                      size="small"
                    >
                      {t('lexicon.addPluralForm', { category })}
                    </Button>
                  ))}
                </Stack>
              )}
            </ListItem>
          ))}
        </List>
      )}
    </Paper>
  );
};
