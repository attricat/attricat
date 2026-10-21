import { useQuery } from '@tanstack/react-query';
import { Stack } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { NewTabIcon } from '../../../components/NewTabIcon';
import { RouterButton } from '../../../components/RouterLink';
import { listBlueprints } from '../../blueprints/api';
import { blueprintQueryKeys } from '../../blueprints/queryKeys';
import { incomingRelationshipSearch } from '../../explorer/search';
import { lexiconText } from '../../lexicon/lexicon';
import { relatedRecordLinkProps } from '../../records/linkedRecord';
import type { ViewComponentDefinition } from './componentTypes';
import { VIEW_COMPONENT_IDS, VIEW_COMPONENT_VERSION } from '../constants';

/**
 * Opens the records linking to this one in the Explorer. The Explorer searches
 * one blueprint at a time, so each relationship gets its own link.
 */
export const IncomingRelationshipListDisplay = ({
  recordId,
  node,
}: {
  recordId: string;
  node: {
    type: 'incoming_relationship_list';
    label: string;
    relationships: { source_blueprint: string; field: string }[];
    page_size: number;
  };
}) => {
  const { t } = useTranslation();
  const several = node.relationships.length > 1;
  // Only several links need the source blueprint's name to tell them apart.
  const blueprints = useQuery({
    queryKey: blueprintQueryKeys.catalogue(),
    queryFn: listBlueprints,
    enabled: several,
  });
  const blueprintName = (code: string) => {
    const blueprint = blueprints.data?.find(
      (candidate) => candidate.code === code,
    );
    return blueprint ? lexiconText(blueprint.name) : code;
  };

  return (
    <Stack direction="row" sx={{ flexWrap: 'wrap', gap: 1 }}>
      {node.relationships.map((relationship) => (
        <RouterButton
          key={`${relationship.source_blueprint}.${relationship.field}`}
          search={incomingRelationshipSearch(recordId, relationship)}
          startIcon={<NewTabIcon />}
          to="/"
          variant="outlined"
          {...relatedRecordLinkProps}
        >
          {several
            ? t('views.incomingRelationshipLink', {
                label: node.label,
                blueprint: blueprintName(relationship.source_blueprint),
              })
            : node.label}
        </RouterButton>
      ))}
    </Stack>
  );
};

export const incomingRelationshipListDisplayComponent = {
  id: VIEW_COMPONENT_IDS.incomingRelationshipListDisplay,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['display'],
  placements: ['incoming_relationship_list'],
  value_types: [],
  allowed_props: [],
  incomingRelationshipRenderer: IncomingRelationshipListDisplay,
} satisfies ViewComponentDefinition;
