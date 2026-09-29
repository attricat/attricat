import { createElement, type ComponentProps } from 'react';
import { findEntityHeading } from '../../views/components/blocks/EntityHeadingDefinition';
import type { HeadingRenderer } from '../../views/components/componentTypes';
import { resolveHeadingRenderer } from '../../views/components/registry';

type Props = ComponentProps<HeadingRenderer>;

/** Renders the heading component configured in an entity's detail view. */
export const EntityHeading = ({
  attributes,
  entityId,
  values,
  view,
}: Props) => {
  const renderer = resolveHeadingRenderer(findEntityHeading(view)?.component);
  return renderer
    ? createElement(renderer, { attributes, entityId, values, view })
    : null;
};
