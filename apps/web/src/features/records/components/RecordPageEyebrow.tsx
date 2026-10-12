import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { BlueprintLink } from '../../blueprints/BlueprintLink';

type Props = {
  blueprint?: { code: string; id: string; name: string; version: number };
  /** Shown until the blueprint has loaded. */
  label: string;
};

/**
 * Page eyebrow naming the kind of record, linking to its blueprint and back
 * to the record list for that blueprint.
 */
export const RecordPageEyebrow = ({ blueprint, label }: Props) => {
  const { t } = useTranslation();
  if (!blueprint) return label;
  return (
    <>
      <BlueprintLink blueprint={blueprint} style={{ textDecoration: 'none' }} />{' '}
      ·{' '}
      <Link
        search={{ blueprint: blueprint.code, version: blueprint.version }}
        to="/"
      >
        {t('records.viewAll')}
      </Link>
    </>
  );
};
