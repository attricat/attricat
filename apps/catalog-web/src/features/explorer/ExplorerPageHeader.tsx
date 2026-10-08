import { useTranslation } from 'react-i18next';
import { PageHeader } from '../../components/PageHeader';
import { RouterButton } from '../../components/RouterLink';
import { ExplorerIcon } from '../../components/systemIcons';

type Props = {
  blueprint: string | undefined;
  /** Name of the selected blueprint, which titles the page. */
  blueprintName: string | undefined;
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
  blueprintName,
  locked,
}: Props) => {
  const { t } = useTranslation();
  return (
    <PageHeader
      actions={
        <RouterButton
          search={createSearch(blueprint, locked)}
          to="/entities/new"
          variant="contained"
        >
          {blueprintName
            ? t('explorer.createBlueprint', { blueprint: blueprintName })
            : t('explorer.create')}
        </RouterButton>
      }
      description={
        blueprintName
          ? t('explorer.blueprintDescription', { blueprint: blueprintName })
          : t('explorer.description')
      }
      icon={ExplorerIcon}
      title={
        blueprintName
          ? t('explorer.blueprintTitle', { blueprint: blueprintName })
          : t('explorer.title')
      }
    />
  );
};
