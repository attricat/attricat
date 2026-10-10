import { createFileRoute } from '@tanstack/react-router';
import i18n from 'i18next';
import { useTranslation } from 'react-i18next';
import { z } from 'zod';
import { LEXICON_MANAGEMENT_NAMESPACE } from '../../features/lexicon/constants';
import { LexiconPage } from '../../features/lexicon/LexiconPage';

const LexiconRouteComponent = () => {
  const { i18n } = useTranslation();
  const search = Route.useSearch();
  const navigate = Route.useNavigate();
  return (
    <LexiconPage
      language={search.language ?? i18n.resolvedLanguage ?? i18n.language}
      onLanguageChange={(language) =>
        void navigate({ search: { language }, replace: true })
      }
    />
  );
};

export const Route = createFileRoute('/manage/lexicon')({
  loader: () => i18n.loadNamespaces(LEXICON_MANAGEMENT_NAMESPACE),
  validateSearch: z.object({ language: z.string().min(1).optional() }),
  component: LexiconRouteComponent,
});
