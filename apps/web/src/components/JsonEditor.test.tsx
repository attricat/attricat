// @vitest-environment jsdom
import { act, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { JsonEditor } from './JsonEditor';
import { jsonEditorMaxHeight, jsonEditorMinHeight } from './jsonEditorSize';

type WrapperProps = {
  defaultLanguage: string;
  defaultValue: string;
  onChange: (value: string | undefined) => void;
  onMount: (editor: typeof fakeEditor) => void;
  options: Record<string, unknown>;
};
let wrapper: WrapperProps;
let contentHeight = 0;
let sizeListener = () => {};
let blurListener = () => {};
const format = vi.fn(() => Promise.resolve());
const fakeEditor = {
  getAction: vi.fn(() => ({ run: format })),
  getContentHeight: () => contentHeight,
  onDidBlurEditorText: (listener: () => void) => {
    blurListener = listener;
  },
  onDidContentSizeChange: (listener: () => void) => {
    sizeListener = listener;
  },
};

vi.mock('./monacoRuntime', () => ({}));
vi.mock('./monacoTheme', () => ({ useMonacoTheme: () => 'light' }));
vi.mock('@monaco-editor/react', () => ({
  Editor: (props: WrapperProps) => {
    wrapper = props;
    return null;
  },
}));

const mount = (props: Partial<Parameters<typeof JsonEditor>[0]> = {}) => {
  const onChange = vi.fn();
  const onBlur = vi.fn();
  render(
    <JsonEditor
      defaultValue={'{\n  "a": 1\n}'}
      label="JSON configuration"
      onBlur={onBlur}
      onChange={onChange}
      {...props}
    />,
  );
  act(() => wrapper.onMount(fakeEditor));
  return { onBlur, onChange };
};

const editorBox = () =>
  screen.getByRole('group', { name: 'JSON configuration' });

describe('JsonEditor', () => {
  beforeEach(() => {
    contentHeight = 0;
    vi.clearAllMocks();
  });

  it('edits JSON and formats what is pasted or typed', () => {
    const { onChange } = mount();
    expect(wrapper.defaultLanguage).toBe('json');
    expect(wrapper.defaultValue).toBe('{\n  "a": 1\n}');
    expect(wrapper.options).toMatchObject({
      ariaLabel: 'JSON configuration',
      formatOnPaste: true,
      formatOnType: true,
      readOnly: false,
    });
    wrapper.onChange('{"a":2}');
    expect(onChange).toHaveBeenCalledWith('{"a":2}');
  });

  it('formats the whole document when it loses focus', () => {
    const { onBlur } = mount();
    act(() => blurListener());
    expect(fakeEditor.getAction).toHaveBeenCalledWith(
      'editor.action.formatDocument',
    );
    expect(format).toHaveBeenCalledOnce();
    expect(onBlur).toHaveBeenCalledOnce();
  });

  it('leaves a read-only document as it is', () => {
    mount({ readOnly: true });
    act(() => blurListener());
    expect(format).not.toHaveBeenCalled();
    expect(wrapper.options.readOnly).toBe(true);
  });

  it('grows with its content between its bounds', () => {
    mount();
    for (const [content, expected] of [
      [40, jsonEditorMinHeight],
      [300, 300],
      [5000, jsonEditorMaxHeight],
    ]) {
      contentHeight = content;
      act(() => sizeListener());
      expect(getComputedStyle(editorBox()).height).toBe(`${expected}px`);
    }
  });

  it('shows an error below the editor', () => {
    mount({ error: 'Enter valid JSON.' });
    expect(screen.getByText('Enter valid JSON.')).toBeTruthy();
  });
});
