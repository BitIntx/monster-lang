use crate::codegen_llvm::emit_program as emit_llvm_program;
use crate::load_program;
use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuildMode {
    Release,
    Debug,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OptLevel {
    O0,
    O1,
    O2,
    O3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetCpu {
    Generic,
    Native,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BuildOptions {
    pub(crate) mode: BuildMode,
    pub(crate) opt_level: OptLevel,
    pub(crate) cpu: TargetCpu,
}

impl BuildMode {
    pub(crate) fn default_opt_level(self) -> OptLevel {
        match self {
            BuildMode::Release => OptLevel::O2,
            BuildMode::Debug => OptLevel::O0,
        }
    }
}

impl OptLevel {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "0" => Ok(Self::O0),
            "1" => Ok(Self::O1),
            "2" => Ok(Self::O2),
            "3" => Ok(Self::O3),
            _ => Err(format!(
                "invalid opt level '{value}', expected 0, 1, 2, or 3"
            )),
        }
    }

    fn as_u8(self) -> u8 {
        match self {
            Self::O0 => 0,
            Self::O1 => 1,
            Self::O2 => 2,
            Self::O3 => 3,
        }
    }

    fn clang_arg(self) -> String {
        format!("-O{}", self.as_u8())
    }

    fn is_optimizing(self) -> bool {
        self != Self::O0
    }
}

impl TargetCpu {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "generic" => Ok(Self::Generic),
            "native" => Ok(Self::Native),
            _ => Err(format!(
                "invalid cpu target '{value}', expected 'generic' or 'native'"
            )),
        }
    }
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            mode: BuildMode::Release,
            opt_level: BuildMode::Release.default_opt_level(),
            cpu: TargetCpu::Generic,
        }
    }
}

pub(crate) fn build_to_binary(input: &str, options: &BuildOptions) -> Result<PathBuf, String> {
    let program = load_program(input)?;
    let llvm_ir = emit_llvm_program(&program)?;

    let input_path = Path::new(input);
    let canonical_input = fs::canonicalize(input_path)
        .map_err(|e| format!("failed to resolve '{}': {}", input_path.display(), e))?;
    let stem = canonical_input
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("invalid input filename: '{input}'"))?;

    let artifact_dir = source_artifact_dir(&canonical_input)?;
    let ll_path = artifact_dir.join(format!("{stem}.ll"));
    let opt_ll_path = artifact_dir.join(format!("{stem}.opt.ll"));
    let out_path = artifact_dir.join(stem);

    fs::create_dir_all(&artifact_dir).map_err(|e| {
        format!(
            "failed to create build artifact directory '{}': {}",
            artifact_dir.display(),
            e
        )
    })?;
    let _artifact_lock = ArtifactLock::acquire(&artifact_dir)?;

    fs::write(&ll_path, llvm_ir)
        .map_err(|e| format!("failed to write '{}': {}", ll_path.display(), e))?;

    verify_llvm_ir(&ll_path)?;

    let compile_input = if options.opt_level.is_optimizing() {
        optimize_llvm_ir(&ll_path, &opt_ll_path, options.opt_level)?;
        verify_llvm_ir(&opt_ll_path)?;
        opt_ll_path.as_path()
    } else {
        if opt_ll_path.exists() {
            fs::remove_file(&opt_ll_path).map_err(|e| {
                format!(
                    "failed to remove stale optimized LLVM IR '{}': {}",
                    opt_ll_path.display(),
                    e
                )
            })?;
        }
        ll_path.as_path()
    };

    compile_to_native(input, compile_input, &out_path, options)?;

    fs::canonicalize(&out_path).map_err(|e| {
        format!(
            "built '{}', but failed to resolve output path '{}': {}",
            input,
            out_path.display(),
            e
        )
    })
}

fn compile_to_native(
    input: &str,
    llvm_input: &Path,
    output_path: &Path,
    options: &BuildOptions,
) -> Result<(), String> {
    let clang = find_tool(&["clang-22", "clang"])
        .ok_or_else(|| "failed to find clang-22 or clang on PATH".to_string())?;

    let mut command = Command::new(&clang);
    command.arg(llvm_input);
    command.arg(options.opt_level.clang_arg());

    if options.mode == BuildMode::Debug {
        command.arg("-g");
    }

    if options.cpu == TargetCpu::Native {
        command.arg("-march=native");
    }

    let output = command
        .arg("-o")
        .arg(output_path)
        .output()
        .map_err(|e| format!("failed to execute {}: {}", clang, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "{} failed while building '{}':\n{}",
            clang, input, stderr
        ));
    }

    Ok(())
}

pub(crate) fn build_artifact_dir() -> Result<PathBuf, String> {
    let cwd = env::current_dir().map_err(|e| format!("failed to get current directory: {e}"))?;
    Ok(cwd.join("target").join("mst"))
}

pub(crate) fn source_artifact_dir(input_path: &Path) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(input_path)
        .map_err(|e| format!("failed to resolve '{}': {}", input_path.display(), e))?;
    let stem = canonical
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("invalid input filename: '{}'", input_path.display()))?;
    let hash = stable_path_hash(&canonical);

    Ok(build_artifact_dir()?.join(format!("{stem}-{hash:016x}")))
}

fn stable_path_hash(path: &Path) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;

    for byte in path.to_string_lossy().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }

    hash
}

struct ArtifactLock {
    path: PathBuf,
}

impl ArtifactLock {
    fn acquire(artifact_dir: &Path) -> Result<Self, String> {
        let lock_path = artifact_dir.join(".lock");

        for _ in 0..600 {
            match fs::create_dir(&lock_path) {
                Ok(()) => return Ok(Self { path: lock_path }),
                Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                    thread::sleep(Duration::from_millis(50));
                }
                Err(e) => {
                    return Err(format!(
                        "failed to acquire build lock '{}': {}",
                        lock_path.display(),
                        e
                    ));
                }
            }
        }

        Err(format!(
            "timed out waiting for build lock '{}'",
            lock_path.display()
        ))
    }
}

impl Drop for ArtifactLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.path);
    }
}

fn optimize_llvm_ir(input: &Path, output: &Path, opt_level: OptLevel) -> Result<(), String> {
    let opt = find_tool(&["opt-22", "opt"])
        .ok_or_else(|| "failed to find opt-22 or opt on PATH".to_string())?;
    let passes = format!("default<O{}>", opt_level.as_u8());

    let command_output = Command::new(&opt)
        .arg(format!("-passes={passes}"))
        .arg("-S")
        .arg(input)
        .arg("-o")
        .arg(output)
        .output()
        .map_err(|e| format!("failed to execute {}: {}", opt, e))?;

    if !command_output.status.success() {
        let stderr = String::from_utf8_lossy(&command_output.stderr);
        return Err(format!(
            "{} failed while optimizing LLVM IR '{}':\n{}",
            opt,
            input.display(),
            stderr
        ));
    }

    Ok(())
}

fn verify_llvm_ir(path: &Path) -> Result<(), String> {
    let opt = find_tool(&["opt-22", "opt"])
        .ok_or_else(|| "failed to find opt-22 or opt on PATH".to_string())?;

    let output = Command::new(&opt)
        .arg("-passes=verify")
        .arg("-disable-output")
        .arg(path)
        .output()
        .map_err(|e| format!("failed to execute {}: {}", opt, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "{} rejected generated LLVM IR '{}':\n{}",
            opt,
            path.display(),
            stderr
        ));
    }

    Ok(())
}

fn find_tool(candidates: &[&str]) -> Option<String> {
    candidates.iter().find_map(|candidate| {
        let status = Command::new(candidate)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .ok()?;

        if status.success() {
            Some((*candidate).to_string())
        } else {
            None
        }
    })
}
