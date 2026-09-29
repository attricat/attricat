export type ParsedDraftProposal = {
  fields: Record<string, string>;
  explanation: string;
  baseValues?: Record<string, string | null>;
};

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

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
