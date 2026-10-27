import { useTranslation } from 'react-i18next';
import { PageHeader } from '../../components/PageHeader';
import { RouterButton } from '../../components/RouterLink';
import { ExplorerIcon } from '../../components/systemIcons';
import { Box } from '@mui/material';
import { BlueprintLink } from '../blueprints/BlueprintLink';
import { BlueprintMigrationBadge } from '../blueprints/BlueprintMigrationBadge';
import { useCan } from '../auth/useCan';
import { capabilities } from '../auth/capabilities';

type Props = {
  blueprint: string | undefined;
  /** Name of the selected blueprint, which titles the page. */
  blueprintName: string | undefined;
  /** The selected blueprint, linked from the eyebrow once it has loaded. */
  blueprintRecord?: { id: string; name: string };
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
  blueprintRecord,
  locked,
}: Props) => {
  const { t } = useTranslation();
  const canCreate = useCan(capabilities.recordsWrite);
  return (
    <PageHeader
      actions={
        canCreate && (
          <RouterButton
            search={createSearch(blueprint, locked)}
            to="/records/new"
            variant="contained"
          >
            {blueprintName
              ? t('explorer.createBlueprint', { blueprint: blueprintName })
              : t('explorer.create')}
          </RouterButton>
        )
      }
      eyebrow={
        blueprintRecord && (
          <Box
            component="span"
            sx={{ alignItems: 'center', display: 'inline-flex', gap: 1 }}
          >
            <BlueprintLink
              blueprint={blueprintRecord}
              style={{ textDecoration: 'none' }}
            />
            <BlueprintMigrationBadge blueprintId={blueprintRecord.id} />
          </Box>
        )
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
