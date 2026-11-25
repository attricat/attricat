import { describe, expect, it } from 'vitest';
import { attributePathFields, frontierAttributes } from './relationshipTargets';

const scalar = (code: string, value_type: 'number' | 'string' = 'number') => ({
  code,
  value_type,
});
const relationship = (code: string, targets: string[]) => ({
  code,
  value_type: 'relationship' as const,
  target_blueprint_codes: targets,
});

describe('frontierAttributes', () => {
  it('merges the fields of every target and names partial ones', () => {
    const frontier = frontierAttributes(
      [
        {
          code: 'sheet',
          attributes: [scalar('thickness_mm'), scalar('width_mm')],
        },
        { code: 'part', attributes: [scalar('thickness_mm')] },
      ],
      attributePathFields,
    );
    expect(frontier.attributes.map((attribute) => attribute.code)).toEqual([
      'thickness_mm',
      'width_mm',
    ]);
    expect(frontier.conflicts.size).toBe(0);
    expect(frontier.partial).toEqual(new Map([['width_mm', ['sheet']]]));
  });

  it('marks fields declared differently as conflicts', () => {
    const frontier = frontierAttributes(
      [
        {
          code: 'sheet',
          attributes: [
            scalar('gauge'),
            relationship('grade', ['grade', 'alloy']),
            relationship('maker', ['brand']),
          ],
        },
        {
          code: 'part',
          attributes: [
            scalar('gauge', 'string'),
            relationship('grade', ['alloy', 'grade']),
            relationship('maker', ['supplier']),
          ],
        },
      ],
      attributePathFields,
    );
    expect([...frontier.conflicts].sort()).toEqual(['gauge', 'maker']);
  });
});
