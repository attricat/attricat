import { useQuery } from '@tanstack/react-query';
import { createElement, type ComponentProps } from 'react';
import { PageTitle } from '../../../components/PageTitle';
import { EntityIcon } from '../../../components/systemIcons';
import { findEntityHeading } from '../../views/components/blocks/EntityHeadingDefinition';
import type { HeadingRenderer } from '../../views/components/componentTypes';
import { resolveHeadingRenderer } from '../../views/components/registry';
import { getEntityLabels } from '../api';
import { displayLabel } from '../entityDisplay';
import { entityQueryKeys } from '../queryKeys';

type Props = ComponentProps<HeadingRenderer>;

/**
 * Titles an entity without a heading component by its display label, the
 * name the Explorer lists it under, or its ID when it has none.
 */
const FallbackEntityHeading = ({
  compact = false,
  entityId,
}: Pick<Props, 'compact' | 'entityId'>) => {
  const label = useQuery({
    queryKey: entityQueryKeys.labels([entityId]),
    queryFn: ({ signal }) => getEntityLabels([entityId], signal),
  });
  // Waits for the label rather than flashing the ID first.
  if (label.isPending) return null;
  const display = label.data?.items.find((item) => item.id === entityId);
  return (
    <PageTitle
      icon={EntityIcon}
      {...(compact && ({ component: 'h2', variant: 'h4' } as const))}
    >
      {displayLabel(display?.display, entityId)}
    </PageTitle>
  );
};

/** Renders the heading component configured in an entity's detail view. */
export const EntityHeading = ({
  attributes,
  compact,
  entityId,
  values,
  view,
}: Props) => {
  const renderer = resolveHeadingRenderer(findEntityHeading(view)?.component);
  return renderer ? (
    createElement(renderer, { attributes, compact, entityId, values, view })
  ) : (
    <FallbackEntityHeading compact={compact} entityId={entityId} />
  );
};
