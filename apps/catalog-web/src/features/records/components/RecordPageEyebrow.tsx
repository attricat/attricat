import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { lexiconText } from '../../lexicon/lexicon';

type Props = {
  blueprint?: { code: string; name: string; version: number };
  /** Shown until the blueprint has loaded. */
  label: string;
};

/**
 * Page eyebrow naming the kind of record, its blueprint, and linking back to
 * the record list for that blueprint.
 */
export const RecordPageEyebrow = ({ blueprint, label }: Props) => {
  const { t } = useTranslation();
  if (!blueprint) return label;
  return (
    <>
      {lexiconText(blueprint.name)} ·{' '}
      <Link
        search={{ blueprint: blueprint.code, version: blueprint.version }}
        to="/"
      >
        {t('records.viewAll')}
      </Link>
    </>
  );
};
