import {
  viewBlockTypes,
  type ViewDefinition,
  type ViewNode,
} from '../../../records/api';
import { isLayoutView } from '../../layoutView';

export const recordHeadingComponentId = 'attricat.record_heading';

export const findRecordHeading = (
  view: ViewDefinition | undefined,
): ViewNode | undefined => {
  if (!isLayoutView(view)) return undefined;
  const visit = (node: ViewNode): ViewNode | undefined => {
    if (
      node.type === viewBlockTypes.stack &&
      node.component?.id === recordHeadingComponentId
    )
      return node;
    if ('children' in node) return node.children.map(visit).find(Boolean);
    if (node.type === viewBlockTypes.tabs)
      return node.tabs
        .flatMap((tab) => tab.children)
        .map(visit)
        .find(Boolean);
    if (node.type === viewBlockTypes.accordion)
      return node.sections
        .flatMap((section) => section.children)
        .map(visit)
        .find(Boolean);
    return undefined;
  };
  return visit(view);
};
