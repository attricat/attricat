// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import '../../i18n';
import { BlueprintChangesDialog } from './BlueprintChangesDialog';

vi.mock('../../components/YamlDiffEditor', () => ({
  YamlDiffEditor: () => <div>diff</div>,
}));

const source = {
  attributeCodes: ['title', 'notes'],
  definition: 'code: car\n',
  version: 3,
};

const renderDialog = (checkedAttributeCodes?: string[]) =>
  render(
    <BlueprintChangesDialog
      checkedAttributeCodes={checkedAttributeCodes}
      definition="code: car\n"
      error={null}
      isPending={false}
      onClose={() => {}}
      onSave={() => {}}
      open
      saveLabel="Save draft"
      source={source}
    />,
  );

describe('BlueprintChangesDialog', () => {
  it('summarizes the attributes a revision adds and removes', async () => {
    renderDialog(['title', 'price']);
    expect(screen.getByText('Changes from version 3')).toBeTruthy();
    expect(screen.getByText('Adds the attribute price.')).toBeTruthy();
    expect(screen.getByRole('alert').textContent).toContain(
      'Removes the attribute notes.',
    );
    expect(await screen.findByText('diff')).toBeTruthy();
  });

  it('waits for a definition that passes the checks', () => {
    renderDialog();
    expect(
      screen.getByText(
        'The attribute summary appears once the definition passes the save checks.',
      ),
    ).toBeTruthy();
  });
});
