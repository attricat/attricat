//! `acli extension new`: a native UI extension, ready to build and pack.

use std::path::{Path, PathBuf};

use serde_json::json;

use crate::error::CliError;

/// The SDK versions a new extension depends on when no local SDK is given.
const EXTENSION_SDK_VERSION: &str = "^0.1.0";
const UI_KIT_PACKAGE_VERSION: &str = "^1.0.0";
/// The UI kit versions the new extension declares it works with.
const UI_KIT_RANGE: &str = "^1.0";
/// The host API range of the new manifest.
const HOST_API_RANGE: &str = ">=1.1.0, <2.0.0";

/// Writes a native Preact extension into `directory`, which must not exist
/// or be empty. With `sdk`, the packages link to that local SDK checkout.
pub(crate) fn scaffold(
    directory: &Path,
    extension_id: &str,
    name: &str,
    sdk: Option<&Path>,
) -> Result<String, CliError> {
    if !extension_id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        || extension_id.is_empty()
        || extension_id.len() > 128
    {
        return Err(CliError::Input(
            "extension ids use 1-128 letters, digits, '.', '_' or '-'".into(),
        ));
    }
    if directory
        .read_dir()
        .is_ok_and(|mut entries| entries.next().is_some())
    {
        return Err(CliError::Input(format!(
            "{} already exists and is not empty",
            directory.display()
        )));
    }
    let dependency = |package: &str, version: &str| match sdk {
        Some(sdk) => format!("link:{}", sdk.join("packages").join(package).display()),
        None => version.to_owned(),
    };
    let files: Vec<(PathBuf, String)> = vec![
        ("manifest.json".into(), manifest(extension_id, name)),
        (
            "package.json".into(),
            pretty(json!({
                "name": extension_id,
                "private": true,
                "type": "module",
                "scripts": {
                    "build": "vite build",
                    "test": "vitest run",
                    "pack": "vite build && attricat-pack"
                },
                "dependencies": {
                    "@attricat/extension": dependency("extension", EXTENSION_SDK_VERSION),
                    "@attricat/ui-kit": dependency("ui-kit", UI_KIT_PACKAGE_VERSION),
                    "preact": "^10.26.4"
                },
                "devDependencies": {
                    "typescript": "^5.8.3",
                    "vite": "^6.3.5",
                    "vitest": "^3.2.4"
                }
            })),
        ),
        ("vite.config.ts".into(), VITE_CONFIG.into()),
        ("tsconfig.json".into(), TSCONFIG.into()),
        ("src/main.tsx".into(), MAIN.into()),
        ("src/Summary.tsx".into(), SUMMARY.into()),
        ("src/UnitEditor.tsx".into(), UNIT_EDITOR.into()),
        ("src/Summary.test.tsx".into(), SUMMARY_TEST.into()),
        ("assets/icon-48.svg".into(), ICON.into()),
        ("README.md".into(), readme(extension_id, name)),
        (".gitignore".into(), "node_modules\ndist\n".into()),
    ];
    for (path, content) in &files {
        let path = directory.join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                CliError::Input(format!("cannot create {}: {error}", parent.display()))
            })?;
        }
        std::fs::write(&path, content).map_err(|error| {
            CliError::Input(format!("cannot write {}: {error}", path.display()))
        })?;
    }
    Ok(pretty(json!({
        "directory": directory.display().to_string(),
        "extension_id": extension_id,
        "files": files.iter().map(|(path, _)| path.display().to_string()).collect::<Vec<_>>(),
        "next": ["pnpm install", "pnpm test", "pnpm pack", "acli extension sideload --file dist/<archive>.tar.zst"]
    })))
}

fn pretty(value: serde_json::Value) -> String {
    format!(
        "{}\n",
        serde_json::to_string_pretty(&value).expect("JSON value")
    )
}

fn manifest(extension_id: &str, name: &str) -> String {
    pretty(json!({
        "manifest_version": 1,
        "name": name,
        "version": "0.1.0",
        "description": format!("{name}: native UI built with the Attricat UI kit."),
        "icons": {"48": "assets/icon-48.svg"},
        "attricat": {"id": extension_id, "host_api": HOST_API_RANGE},
        "permissions": ["client.blueprint_panel", "client.attribute_editor", "storage.extension"],
        "artifacts": [
            {"id": "ui", "kind": "client_native", "path": "dist/ui.js", "ui_kit": UI_KIT_RANGE}
        ],
        "ui": [
            {
                "id": "summary",
                "version": 1,
                "kind": "panel",
                "artifact": "ui",
                "outlet": "blueprint_panel",
                "title": "Summary",
                "translations": {"pl": {"title": "Podsumowanie"}}
            },
            {
                "id": "unit",
                "version": 1,
                "kind": "editor",
                "artifact": "ui",
                "outlet": "attribute_editor",
                "title": "Unit",
                "editor": {
                    "value_types": ["string"],
                    "props": {"units": {"type": "string", "max_length": 120}}
                }
            }
        ]
    }))
}

fn readme(extension_id: &str, name: &str) -> String {
    format!(
        "# {name}\n\n\
         A native UI extension (`{extension_id}`). Its code runs in a Worker and composes\n\
         Attricat UI kit components; Attricat draws them with its own components.\n\n\
         - `src/main.tsx` registers one component per contribution in `manifest.json`.\n\
         - `pnpm test` renders contributions with the SDK test harness.\n\
         - `pnpm pack` builds `dist/ui.js` and packs the archive to side-load.\n\
         - `acli extension dev {extension_id} --build \"pnpm build\" --watch src` rebuilds and\n\
           hot-swaps the Worker in a development server without a new upload or grants.\n\n\
         A blueprint field uses the editor from `views.detail`:\n\n\
         ```yaml\n- type: field\n  field: size\n  editor: {{id: unit, version: 1, props: {{units: \"S,M,L\"}}}}\n```\n"
    )
}

const VITE_CONFIG: &str = "import { defineConfig } from 'vite';
import { attricatExtension } from '@attricat/extension/vite';

// One self-contained Worker bundle at dist/ui.js, within the size budget,
// with the UI kit version it was built against recorded beside it.
export default defineConfig(attricatExtension({ entry: 'src/main.tsx' }));
";

const TSCONFIG: &str = r#"{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "jsx": "react-jsx",
    "jsxImportSource": "preact",
    "strict": true,
    "noEmit": true,
    "skipLibCheck": true
  },
  "include": ["src"]
}
"#;

const MAIN: &str = "import { register } from '@attricat/extension';
import { Summary } from './Summary';
import { UnitEditor } from './UnitEditor';

// Each id is a contribution declared in manifest.json.
register('summary', Summary);
register('unit', UnitEditor);
";

const SUMMARY: &str = "import { useOutletContext, useLocale } from '@attricat/extension';
import { Badge, Stack, Text } from '@attricat/ui-kit';

type BlueprintPanelContext = {
  blueprint_id: string;
  blueprint_version: number;
};

export const Summary = () => {
  const context = useOutletContext<BlueprintPanelContext>();
  const polish = useLocale().startsWith('pl');
  return (
    <Stack gap=\"sm\">
      <Text>{polish ? 'Wersja schematu' : 'Blueprint version'}</Text>
      <Badge tone=\"info\">{String(context.blueprint_version)}</Badge>
    </Stack>
  );
};
";

const UNIT_EDITOR: &str = "import { useEditor } from '@attricat/extension';
import { Select } from '@attricat/ui-kit';

/** Lets a field choose one of the units its blueprint lists in `units`. */
export const UnitEditor = () => {
  const editor = useEditor();
  const units = String(editor.props.units ?? '')
    .split(',')
    .map((unit) => unit.trim())
    .filter(Boolean);
  return (
    <Select
      hideLabel
      label={editor.attribute.name}
      onChange={(event) => editor.propose(event.detail.value)}
      options={units.map((unit) => ({ value: unit, label: unit }))}
      value={typeof editor.value === 'string' ? editor.value : null}
    />
  );
};
";

const SUMMARY_TEST: &str = "import { describe, expect, it } from 'vitest';
import { renderContribution } from '@attricat/extension/testing';
import { Summary } from './Summary';

describe('Summary', () => {
  it('shows the blueprint version', async () => {
    const view = await renderContribution(Summary, {
      context: { blueprint_id: '00000000-0000-4000-8000-000000000000', blueprint_version: 3 },
    });
    expect(view.text()).toContain('3');
  });
});
";

const ICON: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 48 48"><rect width="48" height="48" rx="10" fill="#4b5563"/><path d="M14 24h20M24 14v20" stroke="#fff" stroke-width="4" stroke-linecap="round"/></svg>
"##;
