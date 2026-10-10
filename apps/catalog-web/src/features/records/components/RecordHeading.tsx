import { useQuery } from '@tanstack/react-query';
import { createElement, type ComponentProps } from 'react';
import { PageTitle } from '../../../components/PageTitle';
import { RecordIcon } from '../../../components/systemIcons';
import { findRecordHeading } from '../../views/components/blocks/RecordHeadingDefinition';
import type { HeadingRenderer } from '../../views/components/componentTypes';
import { resolveHeadingRenderer } from '../../views/components/registry';
import { getRecordLabels } from '../api';
import { displayLabel } from '../recordDisplay';
import { recordQueryKeys } from '../queryKeys';

type Props = ComponentProps<HeadingRenderer>;

/**
 * Titles a record without a heading component by its display label, the
 * name the Explorer lists it under, or its ID when it has none.
 */
const FallbackRecordHeading = ({
  compact = false,
  recordId,
}: Pick<Props, 'compact' | 'recordId'>) => {
  const label = useQuery({
    queryKey: recordQueryKeys.labels([recordId]),
    queryFn: ({ signal }) => getRecordLabels([recordId], signal),
  });
  // Waits for the label rather than flashing the ID first.
  if (label.isPending) return null;
  const display = label.data?.items.find((item) => item.id === recordId);
  return (
    <PageTitle
      icon={RecordIcon}
      {...(compact && ({ component: 'h2', variant: 'h4' } as const))}
    >
      {displayLabel(display?.display, recordId)}
    </PageTitle>
  );
};

/** Renders the heading component configured in a record's detail view. */
export const RecordHeading = ({
  attributes,
  compact,
  recordId,
  values,
  view,
}: Props) => {
  const renderer = resolveHeadingRenderer(findRecordHeading(view)?.component);
  return renderer ? (
    createElement(renderer, { attributes, compact, recordId, values, view })
  ) : (
    <FallbackRecordHeading compact={compact} recordId={recordId} />
  );
};
