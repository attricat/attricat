//! Compiling server components. What a compilation costs has little to do with
//! a component's size: 500 KB of nested loops can take 2 GiB. Neither fuel nor an
//! invocation's timeout applies to it, and no check on the bytes can tell such a
//! component from a real one. So production compiles in a child process with its
//! own memory limit, where a hostile component can only end that child.

use std::{path::PathBuf, process::Stdio, sync::Arc, time::Duration};

use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    process::Command,
};
use wasmtime::{Config, Engine, component::Component};

use super::{ExtensionRuntimeError, MAX_INSTANCES};

/// The argument that makes the API binary compile one component and exit.
pub const COMPILE_COMPONENT_COMMAND: &str = "compile-extension-component";
/// The child's address space. A full parent takes about 1 GiB (16 invocations of
/// 32 MiB, the component cache and itself) and `/tmp` 256 MiB, so this keeps the
/// worst case inside the documented 2 GiB. Real components compile in far less: a
/// 17 MB debug build takes under 100 MiB.
const COMPILATION_MEMORY_BYTES: u64 = 512 * 1024 * 1024;
const COMPILATION_TIMEOUT: Duration = Duration::from_secs(120);
const COMPILATION_CPU_SECONDS: u64 = 120;
/// Compiled output the parent accepts, which it then holds in memory.
const MAX_COMPILED_BYTES: u64 = 256 * 1024 * 1024;
/// What one entry of a core module costs an instance of it at most, in the runtime
/// state wasmtime allocates per instance.
const INSTANCE_BYTES_PER_MODULE_ENTRY: usize = 32;
/// The state of one instance. Nothing limits this at run time, and across
/// [`MAX_INSTANCES`] in each of 16 invocations it comes to at most 256 MiB.
const MAX_INSTANCE_STATE_BYTES: usize = 512 * 1024;
const MAX_MODULE_ENTRIES: usize = MAX_INSTANCE_STATE_BYTES / INSTANCE_BYTES_PER_MODULE_ENTRY;
const _: () = assert!(MAX_INSTANCES * MAX_INSTANCE_STATE_BYTES <= 16 * 1024 * 1024);

/// Where components are compiled.
#[derive(Clone, Debug, Default)]
pub enum ComponentCompiler {
    /// In this process. For tests, whose executable is not the API binary.
    #[default]
    InProcess,
    /// In a child process running this executable with
    /// [`COMPILE_COMPONENT_COMMAND`].
    Isolated { executable: PathBuf },
}

/// The engine both sides use. A compiled component loads only into an engine
/// configured the same way.
pub(super) fn engine() -> Result<Engine, ExtensionRuntimeError> {
    let mut config = Config::new();
    config.wasm_component_model(true);
    config.consume_fuel(true);
    config.epoch_interruption(true);
    Engine::new(&config).map_err(super::runtime_error)
}

pub(super) async fn compile<Bytes>(
    compiler: &ComponentCompiler,
    engine: Arc<Engine>,
    bytes: Bytes,
) -> Result<Component, ExtensionRuntimeError>
where
    Bytes: AsRef<[u8]> + Send + 'static,
{
    let invalid = |error| ExtensionRuntimeError::Runtime(format!("invalid component: {error:#}"));
    let joined =
        |error| ExtensionRuntimeError::Runtime(format!("component compilation failed: {error}"));
    check_instance_state(bytes.as_ref()).map_err(invalid)?;
    match compiler {
        // Compiling takes seconds of CPU; keep it off the async workers so it
        // cannot stall unrelated requests.
        ComponentCompiler::InProcess => {
            tokio::task::spawn_blocking(move || Component::new(&engine, &bytes))
                .await
                .map_err(joined)?
                .map_err(invalid)
        }
        ComponentCompiler::Isolated { executable } => {
            let compiled = compile_in_child(executable, bytes.as_ref()).await?;
            tokio::task::spawn_blocking(move || {
                // SAFETY: these bytes are what `Engine::precompile_component` just
                // produced in our own executable, read straight from the pipe to
                // its child, and never stored. `deserialize` rejects output from
                // another version or configuration.
                unsafe { Component::deserialize(&engine, &compiled) }
            })
            .await
            .map_err(joined)?
            .map_err(invalid)
        }
    }
}

/// Refuses modules whose instances would each take too much memory outside the
/// invocation's budget: a module of a million globals, instantiated 30 times, is
/// 1 GiB in a 2 KB archive. Only counts sections, so this is cheap on any input.
fn check_instance_state(component: &[u8]) -> wasmtime::Result<()> {
    use wasmparser::{ElementItems, Parser, Payload};
    let mut entries = 0_usize;
    for payload in Parser::new(0).parse_all(component) {
        let added = match payload? {
            Payload::Version { .. } => {
                entries = 0;
                0
            }
            Payload::TypeSection(section) => section.count() as usize,
            Payload::ImportSection(section) => section.into_imports().count(),
            Payload::TableSection(section) => section.count() as usize,
            Payload::MemorySection(section) => section.count() as usize,
            Payload::GlobalSection(section) => section.count() as usize,
            Payload::ExportSection(section) => section.count() as usize,
            Payload::ElementSection(section) => {
                let mut items = 0_usize;
                for element in section {
                    items += match element?.items {
                        ElementItems::Functions(functions) => functions.count() as usize,
                        ElementItems::Expressions(_, expressions) => expressions.count() as usize,
                    };
                }
                items
            }
            _ => 0,
        };
        entries = entries.saturating_add(added);
        if entries > MAX_MODULE_ENTRIES {
            wasmtime::bail!(
                "a module has more than {MAX_MODULE_ENTRIES} types, imports, globals, tables, memories, exports and table elements together"
            );
        }
    }
    Ok(())
}

async fn compile_in_child(
    executable: &PathBuf,
    component: &[u8],
) -> Result<Vec<u8>, ExtensionRuntimeError> {
    let failed = |reason: &str| {
        ExtensionRuntimeError::Runtime(format!("component compilation failed: {reason}"))
    };
    let mut child = Command::new(executable)
        .arg(COMPILE_COMPONENT_COMMAND)
        // The child runs on untrusted input and needs nothing of the parent's.
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| failed(&error.to_string()))?;
    let mut stdin = child.stdin.take().ok_or_else(|| failed("no stdin"))?;
    let stdout = child.stdout.take().ok_or_else(|| failed("no stdout"))?;
    let run = async {
        let mut compiled = Vec::new();
        let write = async {
            // A child that dies early closes the pipe, which its exit status
            // reports better than this write.
            let _ = stdin.write_all(component).await;
            drop(stdin);
        };
        let read = async {
            stdout
                .take(MAX_COMPILED_BYTES + 1)
                .read_to_end(&mut compiled)
                .await
        };
        let (_, read) = tokio::join!(write, read);
        read.map_err(|error| failed(&error.to_string()))?;
        if compiled.len() as u64 > MAX_COMPILED_BYTES {
            return Err(failed("compiled output is too large"));
        }
        let status = child
            .wait()
            .await
            .map_err(|error| failed(&error.to_string()))?;
        if !status.success() {
            // Running out of memory or CPU ends the child like any invalid input.
            return Err(failed(&format!("the compiler process ended with {status}")));
        }
        Ok(compiled)
    };
    tokio::time::timeout(COMPILATION_TIMEOUT, run)
        .await
        .map_err(|_| failed("timed out"))?
}

/// The child's whole life: reads a component from stdin and writes it compiled to
/// stdout. Call this before starting anything else in the process.
pub fn run_compile_command() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::io::{Read as _, Write as _};
    forbid_core_dumps()?;
    // Only Linux, where production runs, enforces an address space limit.
    let memory = rlimit::Resource::AS.set(COMPILATION_MEMORY_BYTES, COMPILATION_MEMORY_BYTES);
    if cfg!(target_os = "linux") {
        memory?;
    }
    rlimit::Resource::CPU.set(COMPILATION_CPU_SECONDS, COMPILATION_CPU_SECONDS)?;
    let mut component = Vec::new();
    std::io::stdin().read_to_end(&mut component)?;
    let compiled = engine()?.precompile_component(&component)?;
    std::io::stdout().write_all(&compiled)?;
    Ok(())
}

/// Running out of memory aborts the child, which is expected here. Otherwise any
/// extension could write a core dump of up to the memory limit to the host's disk
/// on every compilation.
fn forbid_core_dumps() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    rlimit::Resource::CORE.set(0, 0)?;
    // The kernel ignores that limit when `core_pattern` pipes dumps to a program,
    // as systemd-coredump does.
    #[cfg(target_os = "linux")]
    {
        const NOT_DUMPABLE: libc::c_ulong = 0;
        // SAFETY: PR_SET_DUMPABLE takes one integer and touches no memory of ours.
        if unsafe { libc::prctl(libc::PR_SET_DUMPABLE, NOT_DUMPABLE) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A module with one global section declaring `globals` globals.
    fn module_with_globals(globals: u32) -> Vec<u8> {
        const GLOBAL_SECTION: u8 = 6;
        let leb128 = |mut value: u32| {
            let mut bytes = Vec::new();
            loop {
                let byte = (value & 0x7f) as u8;
                value >>= 7;
                if value == 0 {
                    bytes.push(byte);
                    return bytes;
                }
                bytes.push(byte | 0x80);
            }
        };
        let mut section = leb128(globals);
        for _ in 0..globals {
            // An immutable i32 initialized to 0.
            section.extend([0x7f, 0x00, 0x41, 0x00, 0x0b]);
        }
        let mut module = b"\0asm\x01\0\0\0".to_vec();
        module.push(GLOBAL_SECTION);
        module.extend(leb128(section.len() as u32));
        module.extend(section);
        module
    }

    #[test]
    fn modules_with_too_much_instance_state_are_refused() {
        let most = MAX_MODULE_ENTRIES as u32;
        assert!(check_instance_state(&module_with_globals(most)).is_ok());
        assert!(check_instance_state(&module_with_globals(most + 1)).is_err());
    }
}
