use std::env;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::process::Command;

use crate::process_spawn::configure_tokio_command;

const CODEX_PATH_ENV_VARS: &[&str] = &["CODEX_CLI_PATH", "CODEX_PATH"];
const NODE_PATH_ENV_VARS: &[&str] = &["CODEX_NODE_PATH"];
const NPM_PATH_ENV_VARS: &[&str] = &["CODEX_NPM_PATH"];
const SSH_PATH_ENV_VARS: &[&str] = &["CODEX_SSH_PATH", "SSH_PATH"];

#[cfg(not(target_os = "windows"))]
const COMMON_UNIX_DIRS: &[&str] = &["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"];
#[cfg(not(target_os = "windows"))]
const HOME_RELATIVE_DIRS: &[&str] = &[
    ".local/bin",
    "bin",
    ".npm-global/bin",
    "Library/pnpm",
    ".volta/bin",
    ".yarn/bin",
    ".bun/bin",
    ".asdf/shims",
];

pub async fn new_codex_command() -> Result<Command, String> {
    build_command("codex", CODEX_PATH_ENV_VARS, None, &[], None).await
}

pub async fn resolve_codex_executable_path() -> Result<PathBuf, String> {
    resolve_executable("codex", CODEX_PATH_ENV_VARS, None, &[]).await
}

pub async fn new_node_command(node_path_override: Option<&str>) -> Result<Command, String> {
    let explicit_path = normalize_override_path(node_path_override)?;
    build_command(
        "node",
        NODE_PATH_ENV_VARS,
        explicit_path.as_deref(),
        &[],
        explicit_path.as_deref(),
    )
    .await
}

pub async fn new_npm_command(node_path_override: Option<&str>) -> Result<Command, String> {
    let explicit_node_path = normalize_override_path(node_path_override)?;
    let extra_search_dirs = explicit_node_path
        .as_ref()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .into_iter()
        .collect::<Vec<_>>();

    build_command(
        "npm",
        NPM_PATH_ENV_VARS,
        None,
        &extra_search_dirs,
        explicit_node_path.as_deref(),
    )
    .await
}

pub async fn new_ssh_command() -> Result<Command, String> {
    build_command("ssh", SSH_PATH_ENV_VARS, None, &[], None).await
}

/// Resolve a system executable by override path, known dirs, PATH, and login shell.
/// Used by non-Codex engines (e.g. Claude CLI health checks) that need GUI-app path discovery.
pub async fn resolve_system_executable(
    binary_name: &str,
    path_override: Option<&str>,
) -> Result<PathBuf, String> {
    let explicit_path = match path_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => {
            let path = PathBuf::from(value);
            if is_executable_file(&path) {
                Some(path)
            } else {
                return Err(format!("配置的 {binary_name} 路径无效：{value}"));
            }
        }
        None => None,
    };

    resolve_executable(binary_name, &[], explicit_path.as_deref(), &[]).await
}

async fn build_command(
    binary_name: &str,
    env_vars: &[&str],
    explicit_path: Option<&Path>,
    additional_search_dirs: &[PathBuf],
    node_override: Option<&Path>,
) -> Result<Command, String> {
    let executable_path =
        resolve_executable(binary_name, env_vars, explicit_path, additional_search_dirs).await?;
    let launch_mode =
        determine_launch_mode(&executable_path, node_override, additional_search_dirs).await?;

    Ok(match launch_mode {
        LaunchMode::Direct {
            executable,
            path_dirs,
        } => {
            let mut command = Command::new(&executable);
            configure_tokio_command(&mut command);
            apply_augmented_path(&mut command, path_dirs);
            command
        }
        LaunchMode::ViaNode {
            node_executable,
            script_path,
            path_dirs,
        } => {
            let mut command = Command::new(&node_executable);
            configure_tokio_command(&mut command);
            command.arg(&script_path);
            apply_augmented_path(&mut command, path_dirs);
            command
        }
    })
}

enum LaunchMode {
    Direct {
        executable: PathBuf,
        path_dirs: Vec<PathBuf>,
    },
    ViaNode {
        node_executable: PathBuf,
        script_path: PathBuf,
        path_dirs: Vec<PathBuf>,
    },
}

async fn determine_launch_mode(
    executable_path: &Path,
    node_override: Option<&Path>,
    additional_search_dirs: &[PathBuf],
) -> Result<LaunchMode, String> {
    let executable_dir = executable_path.parent().map(Path::to_path_buf);

    if script_requires_env_node(executable_path) {
        let node_executable = resolve_executable(
            "node",
            NODE_PATH_ENV_VARS,
            node_override,
            additional_search_dirs,
        )
        .await?;
        let node_dir = node_executable.parent().map(Path::to_path_buf);
        return Ok(LaunchMode::ViaNode {
            node_executable,
            script_path: executable_path.to_path_buf(),
            path_dirs: unique_dirs([node_dir, executable_dir]),
        });
    }

    Ok(LaunchMode::Direct {
        executable: executable_path.to_path_buf(),
        path_dirs: unique_dirs([executable_dir]),
    })
}

async fn resolve_executable(
    binary_name: &str,
    env_vars: &[&str],
    explicit_path: Option<&Path>,
    additional_search_dirs: &[PathBuf],
) -> Result<PathBuf, String> {
    if let Some(path) = resolve_explicit_path(binary_name, explicit_path)? {
        return Ok(path);
    }

    if let Some(path) = resolve_from_env_override(env_vars) {
        return Ok(path);
    }

    if let Some(path) = resolve_from_known_paths(binary_name, additional_search_dirs) {
        return Ok(path);
    }

    if let Some(path) = resolve_from_shell(binary_name).await {
        return Ok(path);
    }

    Err(format!(
        "未找到 {binary_name} 可执行文件。请确认已安装并且在终端中执行 `{binary_name} --version` 可以成功。"
    ))
}

fn resolve_explicit_path(
    binary_name: &str,
    explicit_path: Option<&Path>,
) -> Result<Option<PathBuf>, String> {
    let Some(path) = explicit_path else {
        return Ok(None);
    };

    if is_executable_file(path) {
        return Ok(Some(path.to_path_buf()));
    }

    Err(format!(
        "配置的 {binary_name} 路径无效：{}",
        path.to_string_lossy()
    ))
}

fn normalize_override_path(path: Option<&str>) -> Result<Option<PathBuf>, String> {
    match path.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => resolve_explicit_path("node", Some(Path::new(value))),
        None => Ok(None),
    }
}

fn resolve_from_env_override(env_vars: &[&str]) -> Option<PathBuf> {
    env_vars.iter().find_map(|key| {
        env::var_os(key)
            .map(PathBuf::from)
            .filter(|path| is_executable_file(path))
    })
}

fn resolve_from_known_paths(
    binary_name: &str,
    additional_search_dirs: &[PathBuf],
) -> Option<PathBuf> {
    search_dirs(additional_search_dirs)
        .into_iter()
        .find_map(|dir| {
            candidate_binary_names(binary_name)
                .into_iter()
                .map(|name| dir.join(name))
                .find(|path| is_executable_file(path))
        })
}

fn search_dirs(additional_search_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    for dir in additional_search_dirs {
        push_unique_dir(&mut dirs, dir.clone());
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Prefer the active nvm install before process PATH / system bins.
        // GUI-launched apps often inherit a PATH with /usr/local/bin but no nvm,
        // which previously resolved stale codex installs (e.g. 0.132.0 / 0.34.0)
        // instead of the nvm default the user sees in their terminal (0.147.0).
        if let Some(home) = env::var_os("HOME").map(PathBuf::from) {
            push_preferred_nvm_dirs(&mut dirs, &home);
        }
    }

    if let Some(path_var) = env::var_os("PATH") {
        for dir in env::split_paths(&path_var) {
            push_unique_dir(&mut dirs, dir);
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(home) = env::var_os("HOME").map(PathBuf::from) {
            for dir in HOME_RELATIVE_DIRS {
                push_unique_dir(&mut dirs, home.join(dir));
            }

            push_remaining_nvm_dirs(&mut dirs, &home);
            push_fnm_dirs(&mut dirs, &home);
        }

        for dir in COMMON_UNIX_DIRS {
            push_unique_dir(&mut dirs, PathBuf::from(dir));
        }
    }

    dirs
}

/// Active nvm bins only: `NVM_BIN` and the resolved `alias/default`.
#[cfg(not(target_os = "windows"))]
fn push_preferred_nvm_dirs(dirs: &mut Vec<PathBuf>, home: &Path) {
    if let Some(nvm_bin) = env::var_os("NVM_BIN").map(PathBuf::from) {
        push_unique_dir(dirs, nvm_bin);
    }

    let nvm_dir = nvm_root_dir(home);
    if let Some(default_bin) = resolve_nvm_default_bin_dir(&nvm_dir) {
        push_unique_dir(dirs, default_bin);
    }
}

/// Remaining nvm installs, newest Node version first (fallback after PATH).
#[cfg(not(target_os = "windows"))]
fn push_remaining_nvm_dirs(dirs: &mut Vec<PathBuf>, home: &Path) {
    let nvm_dir = nvm_root_dir(home);
    let versions_dir = nvm_dir.join("versions/node");
    let Ok(entries) = fs::read_dir(versions_dir) else {
        return;
    };

    let mut version_bins = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .map(|path| {
            let version_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string();
            (parse_node_version_sort_key(&version_name), path.join("bin"))
        })
        .collect::<Vec<_>>();

    version_bins.sort_by(|left, right| right.0.cmp(&left.0));
    for (_, bin_dir) in version_bins {
        push_unique_dir(dirs, bin_dir);
    }
}

#[cfg(not(target_os = "windows"))]
fn nvm_root_dir(home: &Path) -> PathBuf {
    env::var_os("NVM_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".nvm"))
}

#[cfg(not(target_os = "windows"))]
fn resolve_nvm_default_bin_dir(nvm_dir: &Path) -> Option<PathBuf> {
    let alias_default = fs::read_to_string(nvm_dir.join("alias/default")).ok()?;
    let version_name = resolve_nvm_alias_target(nvm_dir, alias_default.trim(), 0)?;
    let bin_dir = nvm_dir.join("versions/node").join(version_name).join("bin");
    bin_dir.is_dir().then_some(bin_dir)
}

/// Resolve nvm alias chains (`default` → `22.19` → `v22.19.0`, or `lts/*` → …).
#[cfg(not(target_os = "windows"))]
fn resolve_nvm_alias_target(nvm_dir: &Path, name: &str, depth: u8) -> Option<String> {
    if name.is_empty() || depth > 8 {
        return None;
    }

    let versions_dir = nvm_dir.join("versions/node");
    for candidate in [name.to_string(), format!("v{name}")] {
        if versions_dir.join(&candidate).is_dir() {
            return Some(candidate);
        }
    }

    // Partial version like "22.19" should resolve to the newest matching install.
    if let Ok(entries) = fs::read_dir(&versions_dir) {
        let mut matches = entries
            .flatten()
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|version| {
                let stripped = version.trim_start_matches('v');
                stripped == name || stripped.starts_with(&format!("{name}."))
            })
            .collect::<Vec<_>>();
        matches.sort_by(|left, right| {
            parse_node_version_sort_key(right).cmp(&parse_node_version_sort_key(left))
        });
        if let Some(matched) = matches.into_iter().next() {
            return Some(matched);
        }
    }

    let alias_path = nvm_dir.join("alias").join(name);
    let alias_value = fs::read_to_string(alias_path).ok()?;
    resolve_nvm_alias_target(nvm_dir, alias_value.trim(), depth + 1)
}

#[cfg(not(target_os = "windows"))]
fn parse_node_version_sort_key(version: &str) -> Vec<u64> {
    version
        .trim_start_matches('v')
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u64>().ok())
        .collect()
}

#[cfg(not(target_os = "windows"))]
fn push_fnm_dirs(dirs: &mut Vec<PathBuf>, home: &Path) {
    let versions_dir = home.join(".local/share/fnm/node-versions");
    let Ok(entries) = fs::read_dir(versions_dir) else {
        return;
    };

    let mut version_bins = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .map(|path| {
            let version_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string();
            (
                parse_node_version_sort_key(&version_name),
                path.join("installation/bin"),
            )
        })
        .collect::<Vec<_>>();

    version_bins.sort_by(|left, right| right.0.cmp(&left.0));
    for (_, bin_dir) in version_bins {
        push_unique_dir(dirs, bin_dir);
    }
}

fn push_unique_dir(dirs: &mut Vec<PathBuf>, dir: PathBuf) {
    if !dir.as_os_str().is_empty() && !dirs.iter().any(|existing| existing == &dir) {
        dirs.push(dir);
    }
}

fn candidate_binary_names(binary_name: &str) -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        vec![
            format!("{binary_name}.exe"),
            format!("{binary_name}.cmd"),
            format!("{binary_name}.bat"),
            binary_name.to_string(),
        ]
    }

    #[cfg(not(target_os = "windows"))]
    {
        vec![binary_name.to_string()]
    }
}

async fn resolve_from_shell(binary_name: &str) -> Option<PathBuf> {
    let lookups = [
        (
            "/bin/zsh",
            vec!["-lc".to_string(), format!("whence -p {binary_name}")],
        ),
        (
            "/bin/zsh",
            vec!["-ilc".to_string(), format!("whence -p {binary_name}")],
        ),
        (
            "/bin/bash",
            vec!["-lc".to_string(), format!("type -P {binary_name}")],
        ),
        (
            "/bin/bash",
            vec!["-ilc".to_string(), format!("type -P {binary_name}")],
        ),
        (
            "/bin/sh",
            vec!["-lc".to_string(), format!("command -v {binary_name}")],
        ),
    ];

    for (program, args) in lookups {
        if !Path::new(program).exists() {
            continue;
        }

        let mut command = Command::new(program);
        configure_tokio_command(&mut command);
        let output = match command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .await
        {
            Ok(output) => output,
            Err(_) => continue,
        };

        if !output.status.success() {
            continue;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(path) = parse_executable_path_from_output(&stdout) {
            return Some(path);
        }
    }

    None
}

fn script_requires_env_node(path: &Path) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };
    let mut reader = BufReader::new(file);
    let mut first_line = String::new();
    if reader.read_line(&mut first_line).is_err() {
        return false;
    }

    let shebang = first_line.trim();
    shebang.starts_with("#!")
        && shebang.contains("/usr/bin/env")
        && shebang.split_whitespace().any(|token| token == "node")
}

fn parse_executable_path_from_output(output: &str) -> Option<PathBuf> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .rev()
        .map(PathBuf::from)
        .find(|path| is_executable_file(path))
}

fn apply_augmented_path(command: &mut Command, path_dirs: Vec<PathBuf>) {
    let mut combined = Vec::new();
    for dir in path_dirs {
        push_unique_dir(&mut combined, dir);
    }

    if let Some(existing_path) = env::var_os("PATH") {
        for dir in env::split_paths(&existing_path) {
            push_unique_dir(&mut combined, dir);
        }
    }

    if let Ok(joined) = join_paths_lossy(&combined) {
        command.env("PATH", joined);
    }
}

fn join_paths_lossy(paths: &[PathBuf]) -> Result<OsString, env::JoinPathsError> {
    env::join_paths(paths.iter().map(PathBuf::as_path))
}

fn unique_dirs<I>(dirs: I) -> Vec<PathBuf>
where
    I: IntoIterator<Item = Option<PathBuf>>,
{
    let mut unique = Vec::new();
    for dir in dirs.into_iter().flatten() {
        push_unique_dir(&mut unique, dir);
    }
    unique
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };

    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{
        parse_executable_path_from_output, resolve_executable, script_requires_env_node,
        unique_dirs,
    };
    #[cfg(not(target_os = "windows"))]
    use super::{
        parse_node_version_sort_key, resolve_nvm_alias_target, resolve_nvm_default_bin_dir,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn create_temp_dir() -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "codex-ai-cli-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time drift")
                .as_nanos()
        ));
        fs::create_dir_all(&base).expect("create temp dir");
        base
    }

    fn make_executable(path: &PathBuf, content: &str) {
        fs::write(path, content).expect("write temp executable");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(path).expect("metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).expect("chmod temp executable");
        }
    }

    #[test]
    fn shell_output_parser_uses_last_valid_executable_path() {
        let base = create_temp_dir();
        let executable = base.join("codex");
        make_executable(&executable, "#!/bin/sh\n");

        let output = format!(
            "Loading shell profile...\n{}\nnot-a-path\n{}\n",
            executable.display(),
            executable.display()
        );

        let resolved = parse_executable_path_from_output(&output);
        assert_eq!(resolved, Some(executable));

        fs::remove_dir_all(base).expect("remove temp dir");
    }

    #[test]
    fn detects_env_node_shebang() {
        let base = create_temp_dir();
        let script = base.join("codex.js");
        make_executable(&script, "#!/usr/bin/env node\nconsole.log('ok');\n");

        assert!(script_requires_env_node(&script));

        fs::remove_dir_all(base).expect("remove temp dir");
    }

    #[test]
    fn unique_dirs_keeps_first_occurrence_only() {
        let base = create_temp_dir();
        let first = base.join("first");
        let second = base.join("second");
        fs::create_dir_all(&first).expect("create first dir");
        fs::create_dir_all(&second).expect("create second dir");

        let dirs = unique_dirs([
            Some(first.clone()),
            Some(second.clone()),
            Some(first.clone()),
            None,
        ]);

        assert_eq!(dirs, vec![first, second]);

        fs::remove_dir_all(base).expect("remove temp dir");
    }

    #[test]
    fn explicit_override_path_takes_precedence() {
        let base = create_temp_dir();
        let node = base.join("node");
        make_executable(&node, "#!/bin/sh\n");

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");
        let resolved = runtime
            .block_on(resolve_executable("node", &[], Some(Path::new(&node)), &[]))
            .expect("resolve node override");

        assert_eq!(resolved, node);

        fs::remove_dir_all(base).expect("remove temp dir");
    }

    #[test]
    fn additional_search_dirs_help_resolve_neighbor_binaries() {
        let base = create_temp_dir();
        let bin_dir = base.join("bin");
        fs::create_dir_all(&bin_dir).expect("create bin dir");
        let npm = bin_dir.join("npm");
        make_executable(&npm, "#!/bin/sh\n");

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");
        let resolved = runtime
            .block_on(resolve_executable(
                "npm",
                &[],
                None,
                std::slice::from_ref(&bin_dir),
            ))
            .expect("resolve npm from extra search dir");

        assert_eq!(resolved, npm);

        fs::remove_dir_all(base).expect("remove temp dir");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn parse_node_version_sort_key_handles_v_prefix() {
        assert_eq!(parse_node_version_sort_key("v22.19.0"), vec![22, 19, 0]);
        assert_eq!(parse_node_version_sort_key("20.19.2"), vec![20, 19, 2]);
        assert!(parse_node_version_sort_key("v22.19.0") > parse_node_version_sort_key("v20.19.2"));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn nvm_default_alias_prefers_matching_install_over_older_versions() {
        let base = create_temp_dir();
        let nvm_dir = base.join(".nvm");
        let v20_bin = nvm_dir.join("versions/node/v20.19.2/bin");
        let v22_bin = nvm_dir.join("versions/node/v22.19.0/bin");
        fs::create_dir_all(&v20_bin).expect("create v20 bin");
        fs::create_dir_all(&v22_bin).expect("create v22 bin");
        fs::create_dir_all(nvm_dir.join("alias")).expect("create alias dir");
        // nvm often stores default as a partial version like "22.19"
        fs::write(nvm_dir.join("alias/default"), "22.19\n").expect("write default alias");

        let resolved = resolve_nvm_default_bin_dir(&nvm_dir);
        assert_eq!(resolved, Some(v22_bin));

        fs::remove_dir_all(base).expect("remove temp dir");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn nvm_alias_chain_resolves_nested_targets() {
        let base = create_temp_dir();
        let nvm_dir = base.join(".nvm");
        let v22_dir = nvm_dir.join("versions/node/v22.19.0");
        fs::create_dir_all(&v22_dir).expect("create v22 dir");
        fs::create_dir_all(nvm_dir.join("alias/lts")).expect("create lts alias dir");
        fs::write(nvm_dir.join("alias/default"), "lts/*\n").expect("write default");
        fs::write(nvm_dir.join("alias/lts/*"), "22.19.0\n").expect("write lts star");

        let resolved = resolve_nvm_alias_target(&nvm_dir, "lts/*", 0);
        assert_eq!(resolved.as_deref(), Some("v22.19.0"));

        fs::remove_dir_all(base).expect("remove temp dir");
    }
}
