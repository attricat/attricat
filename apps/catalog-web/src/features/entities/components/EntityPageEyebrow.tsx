import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';

type Props = {
  blueprint?: { code: string; version: number };
  label: string;
};

/** Page eyebrow linking back to the entity list for the blueprint. */
export const EntityPageEyebrow = ({ blueprint, label }: Props) => {
  const { t } = useTranslation();
  if (!blueprint) return label;
  return (
    <>
      {label} ·{' '}
      <Link
        search={{ blueprint: blueprint.code, version: blueprint.version }}
        to="/"
      >
        {t('entities.viewAll')}
      </Link>
    </>
  );
};
