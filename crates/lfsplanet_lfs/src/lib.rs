//! The supported ways to run LFS under Wine and Bubblewrap.

#[cfg(not(target_os = "linux"))]
compile_error!("lfsplanet_lfs requires Linux and Bubblewrap");

use std::{
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
    process::Stdio,
};

use anyhow::{Context, ensure};
use tokio::process::Command as ProcessCommand;

const SANDBOX_HOME: &str = "/home/sandbox";
const SANDBOX_DIR: &str = "/sandbox";
const SANDBOX_RUNTIME_DIR: &str = "/run/user/0";
const SANDBOX_LFS_DIR: &str = "/sandbox/LFS";
const SANDBOX_MODS_DIR: &str = "/sandbox/LFS/mods";
const SANDBOX_CACHE_DIR: &str = "/sandbox/LFS/cache";
const SANDBOX_LFS_WINDOWS_DIR: &str = r"Z:\sandbox\LFS";
const SANDBOX_PREFIX: &str = "/sandbox/.wine";
const SANDBOX_SETUP_PATH: &str = "/setup.exe";
const SANDBOX_REPLAY_PATH: &str = "/sandbox/LFS/data/spr/lfsplanet_validator.spr";
const SANDBOX_BATCH_PATH: &str = "/sandbox/LFS/lfsplanet-validator.bat";
const SANDBOX_RESULT_PATH: &str = "/sandbox/lfsplanet-hlvc-result";

const HOST_RUNTIME_PATHS: [&str; 4] = ["/usr", "/bin", "/lib", "/lib64"];
const HOST_CONFIGURATION_PATHS: [&str; 11] = [
    "/etc/alternatives",
    "/etc/fonts",
    "/etc/group",
    "/etc/ld.so.cache",
    "/etc/ld.so.conf",
    "/etc/ld.so.conf.d",
    "/etc/localtime",
    "/etc/machine-id",
    "/etc/nsswitch.conf",
    "/etc/passwd",
    "/etc/wine",
];
const NETWORK_CONFIGURATION_PATHS: [&str; 5] = [
    "/etc/host.conf",
    "/etc/hosts",
    "/etc/resolv.conf",
    "/etc/services",
    "/etc/ssl/certs",
];

/// One complete, valid invocation of LFS or its installer inside the sandbox.
pub enum Command<'a> {
    Initialize {
        wine_prefix: &'a Path,
    },
    Install {
        installation_dir: &'a Path,
        wine_prefix: &'a Path,
        setup: &'a Path,
    },
    Extract {
        installation_dir: &'a Path,
        wine_prefix: &'a Path,
    },
    Update {
        installation_dir: &'a Path,
        wine_prefix: &'a Path,
    },
    Unlock {
        installation_dir: &'a Path,
        wine_prefix: &'a Path,
        username: &'a str,
        code: &'a str,
    },
    Validate {
        installation_dir: &'a Path,
        wine_prefix: &'a Path,
        replay: &'a Path,
        batch: &'a Path,
        result: &'a Path,
    },
}

/// Lifecycle and namespace policy for one Bubblewrap invocation.
#[derive(Clone, Copy, Eq, PartialEq)]
enum SandboxMode {
    /// An ephemeral child must not survive its supervising process.
    Strict,
    /// Retains filesystem, user, IPC, UTS, and cgroup isolation but shares the
    /// host PID and network namespaces so an LFS updater may hand work to a
    /// successor process that outlives Bubblewrap.
    Lax,
}

impl Command<'_> {
    /// Builds the Bubblewrap process without imposing capture, timeout, or
    /// status policy on its caller.
    #[must_use]
    pub fn build(&self, bubblewrap: &Path, wine: &Path) -> ProcessCommand {
        let mut command = ProcessCommand::new(bubblewrap);
        command
            .args(self.arguments(wine))
            .stdin(Stdio::null())
            .kill_on_drop(true);
        command
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the match spells out every complete sandbox invocation"
    )]
    fn arguments(&self, wine: &Path) -> Vec<OsString> {
        let mode = self.sandbox_mode();
        let mut arguments = common_arguments(mode, self.hostname(), wine);

        match self {
            Self::Initialize { wine_prefix } => {
                bind_prefix(&mut arguments, wine_prefix);
                option(&mut arguments, "--chdir", [SANDBOX_DIR]);
                program(
                    &mut arguments,
                    Path::new("/bin/sh"),
                    [
                        OsStr::new("-c"),
                        OsStr::new(
                            "\"$1\" wineboot --init || exit $?; attempts=0; while [ \"$attempts\" -lt 100 ]; do [ -d \"$2/drive_c\" ] && [ -f \"$2/system.reg\" ] && [ -f \"$2/user.reg\" ] && exit 0; attempts=$((attempts + 1)); sleep 0.1; done; echo 'Wine prefix did not finish initialization' >&2; exit 1",
                        ),
                        OsStr::new("wine-prefix-init"),
                        wine.as_os_str(),
                        OsStr::new(SANDBOX_PREFIX),
                    ],
                );
            }
            Self::Install {
                installation_dir,
                wine_prefix,
                setup,
            } => {
                bind_runtime(&mut arguments, installation_dir, wine_prefix);
                option(
                    &mut arguments,
                    "--ro-bind",
                    [setup.as_os_str(), OsStr::new(SANDBOX_SETUP_PATH)],
                );
                option(&mut arguments, "--chdir", [SANDBOX_DIR]);
                program(
                    &mut arguments,
                    wine,
                    [
                        OsString::from(SANDBOX_SETUP_PATH),
                        OsString::from("/S"),
                        OsString::from(format!("/D={SANDBOX_LFS_WINDOWS_DIR}")),
                    ],
                );
            }
            Self::Extract {
                installation_dir,
                wine_prefix,
            } => {
                bind_runtime(&mut arguments, installation_dir, wine_prefix);
                option(&mut arguments, "--chdir", [SANDBOX_LFS_DIR]);
                program(
                    &mut arguments,
                    wine,
                    [
                        OsString::from(format!("{SANDBOX_LFS_DIR}/LFS.exe")),
                        OsString::from("/nogfx=extract"),
                    ],
                );
            }
            Self::Update {
                installation_dir,
                wine_prefix,
            } => {
                add_network_access(&mut arguments, mode);
                bind_runtime(&mut arguments, installation_dir, wine_prefix);
                option(&mut arguments, "--chdir", [SANDBOX_LFS_DIR]);
                program(
                    &mut arguments,
                    wine,
                    [
                        OsString::from(format!("{SANDBOX_LFS_DIR}/LFS.exe")),
                        OsString::from("/nogfx=update"),
                    ],
                );
            }
            Self::Unlock {
                installation_dir,
                wine_prefix,
                username,
                code,
            } => {
                add_network_access(&mut arguments, mode);
                bind_runtime(&mut arguments, installation_dir, wine_prefix);
                option(&mut arguments, "--chdir", [SANDBOX_LFS_DIR]);
                program(
                    &mut arguments,
                    wine,
                    [
                        OsString::from(format!("{SANDBOX_LFS_DIR}/LFS.exe")),
                        OsString::from(format!("/unlock={username}")),
                        OsString::from(format!("/code={code}")),
                    ],
                );
            }
            Self::Validate {
                installation_dir,
                wine_prefix,
                replay,
                batch,
                result,
            } => {
                add_network_access(&mut arguments, mode);
                option(&mut arguments, "--json-status-fd", ["1"]);
                option(&mut arguments, "--dir", [SANDBOX_DIR]);
                ephemeral_overlay(&mut arguments, installation_dir, SANDBOX_LFS_DIR);
                option(&mut arguments, "--dir", [SANDBOX_MODS_DIR]);
                option(
                    &mut arguments,
                    "--bind",
                    [
                        installation_dir.join("mods").as_os_str(),
                        OsStr::new(SANDBOX_MODS_DIR),
                    ],
                );
                option(&mut arguments, "--dir", [SANDBOX_CACHE_DIR]);
                option(
                    &mut arguments,
                    "--bind",
                    [
                        installation_dir.join("cache").as_os_str(),
                        OsStr::new(SANDBOX_CACHE_DIR),
                    ],
                );
                ephemeral_overlay(&mut arguments, wine_prefix, SANDBOX_PREFIX);
                option(
                    &mut arguments,
                    "--ro-bind",
                    [replay.as_os_str(), OsStr::new(SANDBOX_REPLAY_PATH)],
                );
                option(
                    &mut arguments,
                    "--ro-bind",
                    [batch.as_os_str(), OsStr::new(SANDBOX_BATCH_PATH)],
                );
                option(
                    &mut arguments,
                    "--bind",
                    [result.as_os_str(), OsStr::new(SANDBOX_RESULT_PATH)],
                );
                option(&mut arguments, "--chdir", [SANDBOX_LFS_DIR]);
                program(
                    &mut arguments,
                    wine,
                    ["cmd.exe", "/d", "/c", "lfsplanet-validator.bat"],
                );
            }
        }
        arguments
    }

    fn hostname(&self) -> &'static str {
        if matches!(self, Self::Validate { .. }) {
            "lfs-validator"
        } else {
            "lfs-installer"
        }
    }

    fn sandbox_mode(&self) -> SandboxMode {
        if matches!(self, Self::Update { .. }) {
            SandboxMode::Lax
        } else {
            SandboxMode::Strict
        }
    }
}

fn common_arguments(mode: SandboxMode, hostname: &str, wine: &Path) -> Vec<OsString> {
    let mut arguments = Vec::new();
    let isolation_flags = match mode {
        SandboxMode::Strict => &[
            "--unshare-all",
            // Bubblewrap requires this explicitly when disabling nested user
            // namespaces; `--unshare-all` alone is not sufficient.
            "--unshare-user",
            "--die-with-parent",
            "--new-session",
            "--disable-userns",
            "--assert-userns-disabled",
        ][..],
        SandboxMode::Lax => &[
            "--unshare-user",
            "--unshare-ipc",
            "--unshare-uts",
            "--unshare-cgroup-try",
            "--new-session",
            "--disable-userns",
            "--assert-userns-disabled",
        ][..],
    };
    for flag_name in isolation_flags {
        flag(&mut arguments, flag_name);
    }
    option(&mut arguments, "--cap-drop", ["ALL"]);
    option(&mut arguments, "--uid", ["0"]);
    option(&mut arguments, "--gid", ["0"]);
    option(&mut arguments, "--hostname", [hostname]);
    flag(&mut arguments, "--clearenv");
    option(&mut arguments, "--proc", ["/proc"]);
    option(&mut arguments, "--dev", ["/dev"]);
    option(&mut arguments, "--tmpfs", ["/dev/shm"]);
    option(&mut arguments, "--tmpfs", ["/tmp"]);
    option(&mut arguments, "--dir", ["/run"]);
    option(&mut arguments, "--dir", ["/run/user"]);
    private_dir(&mut arguments, SANDBOX_RUNTIME_DIR);
    private_dir(&mut arguments, SANDBOX_HOME);

    for path in HOST_RUNTIME_PATHS.iter().chain(&HOST_CONFIGURATION_PATHS) {
        option(&mut arguments, "--ro-bind-try", [path, path]);
    }
    for (name, value) in [
        ("HOME", SANDBOX_HOME),
        ("LANG", "C.UTF-8"),
        ("LOGNAME", "sandbox"),
        ("PATH", "/usr/local/bin:/usr/bin:/bin"),
        ("USER", "sandbox"),
        ("XDG_RUNTIME_DIR", SANDBOX_RUNTIME_DIR),
        ("WINEDEBUG", "-all,err+all"),
        ("WINEDLLOVERRIDES", "winemenubuilder.exe=d;mscoree,mshtml="),
        ("WINEPREFIX", SANDBOX_PREFIX),
    ] {
        option(&mut arguments, "--setenv", [name, value]);
    }
    option(
        &mut arguments,
        "--ro-bind",
        [wine.as_os_str(), wine.as_os_str()],
    );
    arguments
}

fn bind_runtime(arguments: &mut Vec<OsString>, installation_dir: &Path, wine_prefix: &Path) {
    option(arguments, "--dir", [SANDBOX_DIR]);
    option(
        arguments,
        "--bind",
        [installation_dir.as_os_str(), OsStr::new(SANDBOX_LFS_DIR)],
    );
    bind_prefix(arguments, wine_prefix);
}

fn bind_prefix(arguments: &mut Vec<OsString>, wine_prefix: &Path) {
    option(arguments, "--dir", [SANDBOX_DIR]);
    option(
        arguments,
        "--bind",
        [wine_prefix.as_os_str(), OsStr::new(SANDBOX_PREFIX)],
    );
}

fn ephemeral_overlay(arguments: &mut Vec<OsString>, source: &Path, destination: &str) {
    option(arguments, "--dir", [destination]);
    option(arguments, "--overlay-src", [source.as_os_str()]);
    option(arguments, "--tmp-overlay", [destination]);
}

fn add_network_access(arguments: &mut Vec<OsString>, mode: SandboxMode) {
    if mode == SandboxMode::Strict {
        // `--share-net` is Bubblewrap's override for the network namespace
        // created by `--unshare-all`.
        flag(arguments, "--share-net");
    }
    for path in NETWORK_CONFIGURATION_PATHS {
        option(arguments, "--ro-bind-try", [path, path]);
    }
}

fn private_dir(arguments: &mut Vec<OsString>, path: &str) {
    option(arguments, "--perms", ["0700"]);
    option(arguments, "--dir", [path]);
}

fn flag(arguments: &mut Vec<OsString>, name: &str) {
    arguments.push(name.into());
}

fn option<I, A>(arguments: &mut Vec<OsString>, name: &str, operands: I)
where
    I: IntoIterator<Item = A>,
    A: AsRef<OsStr>,
{
    arguments.push(name.into());
    arguments.extend(
        operands
            .into_iter()
            .map(|operand| operand.as_ref().to_owned()),
    );
}

fn program<I, A>(arguments: &mut Vec<OsString>, executable: &Path, operands: I)
where
    I: IntoIterator<Item = A>,
    A: AsRef<OsStr>,
{
    arguments.push("--".into());
    arguments.push(executable.as_os_str().to_owned());
    arguments.extend(
        operands
            .into_iter()
            .map(|operand| operand.as_ref().to_owned()),
    );
}

pub fn executable(path: &Path, name: &str) -> anyhow::Result<PathBuf> {
    let path = fs::canonicalize(path)
        .with_context(|| format!("failed to find {name} executable {}", path.display()))?;
    ensure!(path.is_file(), "{} is not a file", path.display());
    Ok(path)
}

pub fn validate_prefix(prefix: &Path) -> anyhow::Result<PathBuf> {
    let metadata = fs::symlink_metadata(prefix)
        .with_context(|| format!("Wine prefix does not exist at {}", prefix.display()))?;
    ensure!(
        metadata.file_type().is_dir(),
        "Wine prefix {} must be a real directory, not a symlink",
        prefix.display()
    );
    let prefix = fs::canonicalize(prefix)
        .with_context(|| format!("failed to resolve Wine prefix {}", prefix.display()))?;
    for required in ["drive_c", "system.reg", "user.reg"] {
        ensure!(
            prefix.join(required).exists(),
            "Wine prefix {} is not initialized: missing {required}",
            prefix.display()
        );
    }
    ensure!(
        prefix.join("drive_c").is_dir(),
        "Wine prefix {} has no drive_c directory",
        prefix.display()
    );
    Ok(prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(arguments: &[OsString]) -> Vec<&str> {
        arguments
            .iter()
            .map(|argument| argument.to_str().unwrap())
            .collect()
    }

    fn has(arguments: &[&str], expected: &[&str]) -> bool {
        arguments
            .windows(expected.len())
            .any(|window| window == expected)
    }

    #[test]
    fn prefix_initialization_waits_for_completion_markers() {
        let arguments = Command::Initialize {
            wine_prefix: Path::new("/srv/lfs/.wine"),
        }
        .arguments(Path::new("/usr/bin/wine"));
        let arguments = strings(&arguments);

        assert_eq!(
            &arguments[arguments.len() - 7..],
            [
                "--",
                "/bin/sh",
                "-c",
                "\"$1\" wineboot --init || exit $?; attempts=0; while [ \"$attempts\" -lt 100 ]; do [ -d \"$2/drive_c\" ] && [ -f \"$2/system.reg\" ] && [ -f \"$2/user.reg\" ] && exit 0; attempts=$((attempts + 1)); sleep 0.1; done; echo 'Wine prefix did not finish initialization' >&2; exit 1",
                "wine-prefix-init",
                "/usr/bin/wine",
                SANDBOX_PREFIX,
            ]
        );
    }

    #[test]
    fn validation_uses_an_ephemeral_installation_and_writable_mod_and_cache_directories() {
        let command = Command::Validate {
            installation_dir: Path::new("/srv/lfs/0.7"),
            wine_prefix: Path::new("/srv/lfs/.wine"),
            replay: Path::new("/tmp/replay.spr"),
            batch: Path::new("/tmp/validate.bat"),
            result: Path::new("/tmp/result"),
        };
        let arguments = command.arguments(Path::new("/usr/bin/wine"));
        let arguments = strings(&arguments);

        assert!(has(&arguments, &["--hostname", "lfs-validator"]));
        assert!(has(&arguments, &["--overlay-src", "/srv/lfs/0.7"]));
        assert!(has(&arguments, &["--tmp-overlay", SANDBOX_LFS_DIR]));
        assert!(has(&arguments, &["--dir", SANDBOX_MODS_DIR]));
        assert!(has(
            &arguments,
            &["--bind", "/srv/lfs/0.7/mods", SANDBOX_MODS_DIR]
        ));
        assert!(has(&arguments, &["--dir", SANDBOX_CACHE_DIR]));
        assert!(has(
            &arguments,
            &["--bind", "/srv/lfs/0.7/cache", SANDBOX_CACHE_DIR]
        ));
        assert!(has(&arguments, &["--overlay-src", "/srv/lfs/.wine"]));
        assert!(has(&arguments, &["--tmp-overlay", SANDBOX_PREFIX]));
        assert!(has(
            &arguments,
            &["--ro-bind", "/tmp/replay.spr", SANDBOX_REPLAY_PATH]
        ));
        assert!(has(
            &arguments,
            &["--ro-bind", "/tmp/validate.bat", SANDBOX_BATCH_PATH]
        ));
        assert!(has(
            &arguments,
            &["--bind", "/tmp/result", SANDBOX_RESULT_PATH]
        ));
        assert!(has(&arguments, &["--json-status-fd", "1"]));
        assert!(arguments.contains(&"--share-net"));
        assert!(has(
            &arguments,
            &["--ro-bind-try", "/etc/resolv.conf", "/etc/resolv.conf"]
        ));
        assert_eq!(
            &arguments[arguments.len() - 6..],
            [
                "--",
                "/usr/bin/wine",
                "cmd.exe",
                "/d",
                "/c",
                "lfsplanet-validator.bat",
            ]
        );
    }

    #[test]
    fn network_access_is_available_for_operations_that_may_need_downloads() {
        let extract = Command::Extract {
            installation_dir: Path::new("/srv/lfs/0.7"),
            wine_prefix: Path::new("/srv/lfs/.wine"),
        }
        .arguments(Path::new("/usr/bin/wine"));
        let update = Command::Update {
            installation_dir: Path::new("/srv/lfs/0.7"),
            wine_prefix: Path::new("/srv/lfs/.wine"),
        }
        .arguments(Path::new("/usr/bin/wine"));
        let unlock = Command::Unlock {
            installation_dir: Path::new("/srv/lfs/0.7"),
            wine_prefix: Path::new("/srv/lfs/.wine"),
            username: "driver",
            code: "secret",
        }
        .arguments(Path::new("/usr/bin/wine"));
        let extract = strings(&extract);
        let update = strings(&update);
        let unlock = strings(&unlock);

        assert!(!extract.contains(&"--share-net"));
        assert!(!has(
            &extract,
            &["--ro-bind-try", "/etc/resolv.conf", "/etc/resolv.conf"]
        ));
        assert!(!update.contains(&"--share-net"));
        assert!(has(
            &update,
            &["--ro-bind-try", "/etc/resolv.conf", "/etc/resolv.conf"]
        ));
        assert!(update.contains(&"--unshare-user"));
        assert!(update.contains(&"--unshare-ipc"));
        assert!(update.contains(&"--unshare-uts"));
        assert!(!update.contains(&"--unshare-all"));
        assert!(!update.contains(&"--unshare-pid"));
        assert!(!update.contains(&"--die-with-parent"));
        assert!(has(&update, &["/sandbox/LFS/LFS.exe", "/nogfx=update"]));
        assert!(unlock.contains(&"--share-net"));
        assert!(has(
            &unlock,
            &["--ro-bind-try", "/etc/resolv.conf", "/etc/resolv.conf"]
        ));
    }

    #[test]
    fn installation_files_and_the_shared_prefix_are_bound_separately() {
        let command = Command::Install {
            installation_dir: Path::new("/srv/lfs/0.8"),
            wine_prefix: Path::new("/srv/lfs/.wine"),
            setup: Path::new("/tmp/setup.exe"),
        };
        let arguments = command.arguments(Path::new("/usr/bin/wine"));
        let arguments = strings(&arguments);

        assert!(arguments.contains(&"--unshare-user"));
        assert!(arguments.contains(&"--disable-userns"));
        assert!(!has(&arguments, &["--setenv", "TMPDIR", "/tmp"]));
        assert!(has(&arguments, &["--uid", "0"]));
        assert!(has(&arguments, &["--gid", "0"]));
        assert!(has(&arguments, &["--dir", SANDBOX_RUNTIME_DIR]));
        assert!(has(
            &arguments,
            &["--setenv", "XDG_RUNTIME_DIR", SANDBOX_RUNTIME_DIR]
        ));
        assert!(has(
            &arguments,
            &["--bind", "/srv/lfs/0.8", SANDBOX_LFS_DIR]
        ));
        assert!(has(
            &arguments,
            &["--bind", "/srv/lfs/.wine", SANDBOX_PREFIX]
        ));
        assert!(has(
            &arguments,
            &["/setup.exe", "/S", "/D=Z:\\sandbox\\LFS"]
        ));
    }

    #[test]
    fn requires_an_initialized_prefix() {
        let root = tempfile::tempdir().unwrap();
        assert!(validate_prefix(root.path()).is_err());

        fs::create_dir(root.path().join("drive_c")).unwrap();
        fs::write(root.path().join("system.reg"), b"registry").unwrap();
        assert!(validate_prefix(root.path()).is_err());

        fs::write(root.path().join("user.reg"), b"registry").unwrap();
        validate_prefix(root.path()).unwrap();
    }
}
