import { useTranslation } from 'react-i18next';
import { PageHeader } from '../../components/PageHeader';
import { RouterButton } from '../../components/RouterLink';

type Props = {
  blueprint: string | undefined;
  /** Name of the blueprint the Explorer is locked to, if any. */
  lockedBlueprintName: string | undefined;
  locked: boolean;
};

const createSearch = (blueprint: string | undefined, locked: boolean) => {
  if (!blueprint) return {};
  // A locked Explorer keeps the create form on its blueprint; otherwise the
  // selected blueprint only preselects the form.
  return locked ? { blueprint, locked: true } : { blueprint };
};

export const ExplorerPageHeader = ({
  blueprint,
  locked,
  lockedBlueprintName,
}: Props) => {
  const { t } = useTranslation();
  const lockedName = locked ? lockedBlueprintName : undefined;
  return (
    <PageHeader
      actions={
        <RouterButton
          search={createSearch(blueprint, locked)}
          to="/entities/new"
          variant="contained"
        >
          {lockedName
            ? t('explorer.createBlueprint', { blueprint: lockedName })
            : t('explorer.create')}
        </RouterButton>
      }
      description={
        lockedName
          ? t('explorer.blueprintDescription', { blueprint: lockedName })
          : t('explorer.description')
      }
      title={
        lockedName
          ? t('explorer.blueprintTitle', { blueprint: lockedName })
          : t('explorer.title')
      }
    />
  );
};
