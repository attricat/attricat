import {
  viewBlockTypes,
  type ViewDefinition,
  type ViewNode,
} from '../../../records/api';

export const recordHeadingComponentId = 'catalog.record_heading';

export const findRecordHeading = (
  view: ViewDefinition | undefined,
): ViewNode | undefined => {
  if (
    !view ||
    view.type === viewBlockTypes.table ||
    view.type === viewBlockTypes.dropdownOption ||
    view.type === viewBlockTypes.extensionLayout
  )
    return undefined;
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
