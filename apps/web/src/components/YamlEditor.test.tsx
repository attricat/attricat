// @vitest-environment jsdom
import { act, render } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { YamlEditor } from './YamlEditor';

type WrapperProps = {
  onChange: (value: string) => void;
  onMount: (editor: typeof fakeEditor, monaco: object) => void;
  value: string;
};
let wrapper: WrapperProps;
let text = '';
const fakeEditor = {
  executeEdits: vi.fn((_source: string, edits: { text: string }[]) => {
    text = edits[0].text;
    return true;
  }),
  getModel: () => ({ getFullModelRange: () => ({}) }),
  getValue: () => text,
  pushUndoStop: vi.fn(),
  setValue: vi.fn(),
};

vi.mock('./monacoRuntime', () => ({}));
vi.mock('./monacoTheme', () => ({ useMonacoTheme: () => 'light' }));
vi.mock('./yamlLanguage', () => ({
  configureYaml: vi.fn(),
  yamlLanguageId: 'yaml',
}));
vi.mock('@monaco-editor/react', () => ({
  Editor: (props: WrapperProps) => {
    wrapper = props;
    return null;
  },
}));

const editor = (value: string, onChange = vi.fn(), readOnly = false) => (
  <YamlEditor
    height="100px"
    onChange={onChange}
    readOnly={readOnly}
    value={value}
  />
);

const type = (next: string) => {
  text = next;
  act(() => wrapper.onChange(next));
};

describe('YamlEditor', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    text = 'code: x';
  });

  it('keeps what was typed since when a render carries older text', () => {
    const { rerender } = render(editor('code: x'));
    act(() => wrapper.onMount(fakeEditor, {}));

    type('code: a');
    type('code: ab');
    // The parent catches up with the first keystroke after the second landed.
    rerender(editor('code: a'));
    expect(wrapper.value).toBe('code: x');
    expect(fakeEditor.executeEdits).not.toHaveBeenCalled();
    expect(text).toBe('code: ab');

    rerender(editor('code: ab'));
    expect(fakeEditor.executeEdits).not.toHaveBeenCalled();
  });

  it('applies a value from elsewhere, such as a chosen template', () => {
    const { rerender } = render(editor('code: x'));
    act(() => wrapper.onMount(fakeEditor, {}));
    type('code: a');
    rerender(editor('code: a'));

    rerender(editor('code: template'));
    expect(text).toBe('code: template');
    expect(fakeEditor.executeEdits).toHaveBeenCalledTimes(1);
    expect(wrapper.value).toBe('code: template');
  });

  it('applies going back to text it had already sent once caught up', () => {
    const { rerender } = render(editor('code: x'));
    act(() => wrapper.onMount(fakeEditor, {}));
    type('code: a');
    rerender(editor('code: a'));

    rerender(editor('code: x'));
    expect(text).toBe('code: x');
  });

  it('sets the text directly while read-only', () => {
    const { rerender } = render(editor('code: x', vi.fn(), true));
    act(() => wrapper.onMount(fakeEditor, {}));

    rerender(editor('code: y', vi.fn(), true));
    expect(fakeEditor.setValue).toHaveBeenCalledWith('code: y');
    expect(fakeEditor.executeEdits).not.toHaveBeenCalled();
  });
});
