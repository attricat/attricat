import { isJsonObject as isRecord } from '../../api/jsonObject';

export type ParsedDraftProposal = {
  fields: Record<string, string>;
  explanation: string;
  baseValues?: Record<string, string | null>;
};

export const parseDraftProposal = (
  content: unknown,
): ParsedDraftProposal | null => {
  if (!isRecord(content) || !isRecord(content.draft_proposal)) return null;
  const proposal = content.draft_proposal;
  if (!isRecord(proposal.fields)) return null;
  const fields = Object.fromEntries(
    Object.entries(proposal.fields).filter(
      (entry): entry is [string, string] => typeof entry[1] === 'string',
    ),
  );
  // A proposal without values is an ordinary reply, not something to apply.
  if (Object.keys(fields).length === 0) return null;
  return {
    fields,
    baseValues: isRecord(proposal.base_values)
      ? Object.fromEntries(
          Object.entries(proposal.base_values).filter(
            (entry): entry is [string, string | null] =>
              typeof entry[1] === 'string' || entry[1] === null,
          ),
        )
      : undefined,
    explanation:
      typeof proposal.explanation === 'string' ? proposal.explanation : '',
  };
};
