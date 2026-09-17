import { describe, expect, it } from 'vitest';
import { isHiddenByDefault } from './attribute-visibility';

describe('isHiddenByDefault', () => {
  it('applies the global hidden tag to every surface', () => {
    for (const surface of ['form', 'detail', 'explorer', 'metadata'] as const) {
      expect(isHiddenByDefault({ tags: ['hidden'] }, surface)).toBe(true);
    }
  });

  it('applies scoped tags only to their matching surface', () => {
    expect(isHiddenByDefault({ tags: ['hidden:form'] }, 'form')).toBe(true);
    expect(isHiddenByDefault({ tags: ['hidden:form'] }, 'detail')).toBe(false);
    expect(isHiddenByDefault({ tags: ['hidden:explorer'] }, 'explorer')).toBe(
      true,
    );
    expect(isHiddenByDefault({ tags: ['hidden:metadata'] }, 'metadata')).toBe(
      true,
    );
  });

  it('leaves untagged attributes visible by default', () => {
    expect(isHiddenByDefault({}, 'detail')).toBe(false);
  });
});
