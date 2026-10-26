import { attributeLabel } from '../records/recordDisplay';

export type FindingAttribute = { code: string; name?: unknown };

/** Names the attribute a finding is about by its label instead of its code. */
export const findingMessage = (
  message: string,
  attributeCode: string | undefined,
  attributes: readonly FindingAttribute[],
) => {
  const attribute = attributes.find((item) => item.code === attributeCode);
  return attribute
    ? message.replaceAll(`'${attribute.code}'`, attributeLabel(attribute))
    : message;
};
