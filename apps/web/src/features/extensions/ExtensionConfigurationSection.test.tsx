// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '../../i18n';
import type { JsonEditorProps } from '../../components/JsonEditor';
import { ExtensionConfigurationSection } from './ExtensionConfigurationSection';
import {
  configureExtension,
  type ExtensionInstallation,
} from './managementApi';

vi.mock('./managementApi', () => ({ configureExtension: vi.fn() }));
// Monaco has no layout in jsdom; Playwright covers the real editor.
vi.mock('../../components/JsonEditor', () => ({
  JsonEditor: ({
    defaultValue,
    error,
    label,
    onChange,
    readOnly,
  }: JsonEditorProps) => (
    <>
      <textarea
        aria-label={label}
        defaultValue={defaultValue}
        onChange={(event) => onChange(event.target.value)}
        readOnly={readOnly}
      />
      {error && <p>{error}</p>}
    </>
  ),
}));

const installation = {
  configuration: { region: 'eu', retries: 2 },
} as unknown as ExtensionInstallation;

const renderSection = (canManage = true) =>
  render(
    <QueryClientProvider client={new QueryClient()}>
      <ExtensionConfigurationSection
        canManage={canManage}
        extensionId="example"
        installation={installation}
      />
    </QueryClientProvider>,
  );

describe('ExtensionConfigurationSection', () => {
  afterEach(() => vi.clearAllMocks());

  it('starts from the formatted configuration and saves the parsed JSON', async () => {
    vi.mocked(configureExtension).mockResolvedValue(installation);
    renderSection();
    const editor = (await screen.findByLabelText(
      'JSON configuration',
    )) as HTMLTextAreaElement;
    expect(editor.value).toBe('{\n  "region": "eu",\n  "retries": 2\n}');
    fireEvent.change(editor, { target: { value: '{"region":"us"}' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save configuration' }));
    await waitFor(() =>
      expect(configureExtension).toHaveBeenCalledWith('example', {
        region: 'us',
      }),
    );
  });

  it('keeps invalid JSON from the server', async () => {
    renderSection();
    fireEvent.change(await screen.findByLabelText('JSON configuration'), {
      target: { value: '{"region":' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save configuration' }));
    expect(
      await screen.findByText('Configuration must be valid JSON.'),
    ).toBeTruthy();
    expect(configureExtension).not.toHaveBeenCalled();
  });

  it('shows the configuration read-only without the manage permission', async () => {
    renderSection(false);
    expect(
      (
        (await screen.findByLabelText(
          'JSON configuration',
        )) as HTMLTextAreaElement
      ).readOnly,
    ).toBe(true);
    expect(
      (
        screen.getByRole('button', {
          name: 'Save configuration',
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
  });
});
