import type { Attribute, BlueprintWithAttributes } from '../../records/api';
import type { IncomingRelationshipListNode } from '../schemas';

type Node = IncomingRelationshipListNode;

/**
 * The columns of the table: the node's `columns`, or every attribute of the
 * source blueprint but the relationship back to this record.
 */
export const incomingTableColumns = (
  blueprint: BlueprintWithAttributes,
  node: Pick<Node, 'columns'>,
  field: string,
): Attribute[] => {
  const attributes = blueprint.attributes.filter(
    (attribute) => attribute.code !== field,
  );
  if (!node.columns?.length) return attributes;
  return node.columns.flatMap((code) => {
    const attribute = attributes.find((candidate) => candidate.code === code);
    return attribute ? [attribute] : [];
  });
};
