import { describe, expect, it } from 'vitest';
import '../../i18n';
import { blueprintTemplates } from '../blueprints/blueprintEditorUtils';
import { reusableAttributeTemplates } from '../reusable-attributes/templates';
import { ruleTemplates } from '../rules/templates';
import { starterWorkflowDefinition } from '../workflows/constants';
import { definitionKinds } from './constants';
import { definitionDiagnostics } from './definitionDiagnostics';

const header = `format_version = 1
code = "product"
name = "Product"
kind = "record"
`;
const attribute = `
[[attributes]]
code = "name"
value_type = "string"
`;

/** Applies a diagnostic's quick fix to the text it was reported for. */
const applyFix = (text: string, message = /./) => {
  const fix = definitionDiagnostics(definitionKinds.blueprint, text).find(
    (diagnostic) => diagnostic.fix && message.test(diagnostic.message),
  )?.fix;
  if (!fix) return undefined;
  return [...fix.edits]
    .sort((left, right) => right.range.start - left.range.start)
    .reduce(
      (result, { range, text: replacement }) =>
        result.slice(0, range.start) + replacement + result.slice(range.end),
      text,
    );
};

const workflowHead = `format_version = 2
code = "tagging"
name = "Tagging"

[[actions]]
type = "system_tags_add"
tags = ["new"]
`;

const diagnose = (text: string) =>
  definitionDiagnostics(definitionKinds.blueprint, text).map(
    ({ message, range }) => ({
      message,
      source: text.slice(range.start, range.end),
    }),
  );

describe('definitionDiagnostics', () => {
  it('accepts the built-in templates', () => {
    for (const template of blueprintTemplates)
      expect(diagnose(template.definition)).toEqual([]);
    for (const template of reusableAttributeTemplates)
      expect(
        definitionDiagnostics(
          definitionKinds.reusableAttribute,
          template.definition,
        ),
      ).toEqual([]);
    for (const template of ruleTemplates)
      expect(
        definitionDiagnostics(definitionKinds.rule, template.definition),
      ).toEqual([]);
  });

  it('reports TOML syntax errors on their line', () => {
    const [diagnostic] = diagnose(`${header}name = \n`);
    expect(diagnostic.message).toMatch(/^Invalid TOML:/);
  });

  it('flags unknown keys and suggests close matches', () => {
    expect(diagnose(`${header}${attribute}valu_type = "string"\n`)).toEqual([
      {
        message: 'Unknown key “valu_type”. Did you mean “value_type”?',
        source: 'valu_type',
      },
    ]);
    expect(diagnose(`${header}colour = "red"\n${attribute}`)).toEqual([
      { message: 'Unknown key “colour”.', source: 'colour' },
    ]);
  });

  it('flags invalid values on the value key', () => {
    expect(diagnose(header.replace('"record"', '"thing"') + attribute)).toEqual(
      [
        {
          message: 'Expected one of: "record", "mixin".',
          source: 'kind',
        },
      ],
    );
    expect(
      diagnose(`${header}${attribute.replace('"string"', '"text"')}`),
    ).toEqual([expect.objectContaining({ source: 'value_type' })]);
    expect(
      diagnose(`${header.replace('"product"', '"my product"')}${attribute}`),
    ).toEqual([
      {
        message: 'Use only ASCII letters, numbers, hyphens, and underscores.',
        source: 'code',
      },
    ]);
  });

  it('reports only the selected block variant', () => {
    expect(
      diagnose(
        `${header}[views.detail]\ntype = "stack"\nchildren = [{ type = "field", field = "name", label = "Name" }]\n${attribute}`,
      ),
    ).toEqual([{ message: 'Unknown key “label”.', source: 'label' }]);
    expect(
      diagnose(
        `${header}[views.detail]\ntype = "stak"\nchildren = []\n${attribute}`,
      ),
    ).toEqual([
      expect.objectContaining({
        message: expect.stringContaining('"stack"'),
        source: 'type',
      }),
    ]);
  });

  it('points missing keys at the owning table', () => {
    expect(
      diagnose(`${header}\n[[attributes]]\nvalue_type = "string"\n`),
    ).toEqual([
      {
        message: 'Missing required key “code”.',
        source: '[[attributes]]',
      },
    ]);
  });

  it('fixes unknown keys with their closest match', () => {
    expect(applyFix(`${header}unrelated = 1\n`)).toBeUndefined();
    expect(applyFix(header.replace('name =', 'nme ='), /^Unknown key/)).toBe(
      header,
    );
  });

  it('fixes values with the closest allowed one', () => {
    expect(applyFix(header.replace('"record"', '"recrd"'))).toBe(header);
  });

  it('fixes missing keys by adding them to their table', () => {
    expect(applyFix(`${header}\n[[attributes]]\nvalue_type = "string"\n`)).toBe(
      `${header}\n[[attributes]]\ncode = ""\nvalue_type = "string"\n`,
    );
  });

  describe('workflows', () => {
    const diagnoseWorkflow = (text: string) =>
      definitionDiagnostics(definitionKinds.workflow, text).map(
        ({ message, range }) => ({
          message,
          source: text.slice(range.start, range.end),
        }),
      );

    it('accepts the starter workflow', () => {
      expect(diagnoseWorkflow(starterWorkflowDefinition)).toEqual([]);
    });

    it('validates untyped event triggers against their own branch', () => {
      expect(
        diagnoseWorkflow(
          `${workflowHead}\n[[triggers]]\nevent_type = "record.created.v1"\nbogus = 1\n`,
        ),
      ).toEqual([{ message: 'Unknown key “bogus”.', source: 'bogus' }]);
    });

    it('validates typed triggers against the tagged branch', () => {
      expect(
        diagnoseWorkflow(
          `${workflowHead}\n[[triggers]]\ntype = "schedule"\ncron = "0 0 3 * * *"\ntimezone = "UTC"\n`,
        ),
      ).toEqual([
        {
          message: 'Missing required key “target_record_id”.',
          source: '[[triggers]]',
        },
      ]);
    });

    it('explains cross-field rules on the key they forbid', () => {
      expect(
        diagnoseWorkflow(
          `${workflowHead.replace('format_version = 2', 'format_version = 1')}\n[[triggers]]\ntype = "manual"\n`,
        ),
      ).toEqual([
        {
          message:
            '“type” is not allowed here. Format version 1 accepts only record event triggers.',
          source: 'type',
        },
      ]);
    });

    it('describes format errors with the field description', () => {
      const [diagnostic] = diagnoseWorkflow(
        `${workflowHead}\n[[triggers]]\ntype = "schedule"\ncron = "daily"\ntimezone = "UTC"\ntarget_record_id = "00000000-0000-0000-0000-000000000000"\n`,
      );
      expect(diagnostic?.message).toMatch(/^Invalid format\. Six-field cron/);
    });
  });
});
