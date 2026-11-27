use std::io::Write;

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::{Command, Output, Stdio};

use anyhow::{Context, Result, bail};

const GIT_HELP: &str = "git may not be installed; see https://github.com/brandonfglx/bd/blob/main/README.md#troubleshooting";

pub fn add_all_except<F>(root: &Path, exclude: F) -> Result<()>
where
	F: Fn(&str) -> bool,
{
	let output = run_checked(
		root,
		&[
			"ls-files",
			"-t",
			"-z",
			"--cached",
			"--others",
			"--exclude-standard",
			"--",
			".",
		],
	)?;

	let mut paths = Vec::new();
	for entry in output.stdout.split(|byte| *byte == 0) {
		if entry.len() < 3 || entry[1] != b' ' {
			continue;
		}

		if entry[0] == b'S' {
			continue;
		}

		let path = &entry[2..];
		if exclude(&String::from_utf8_lossy(path)) {
			continue;
		}

		paths.extend_from_slice(path);
		paths.push(0);
	}

	if paths.is_empty() {
		return Ok(());
	}

	add_paths(root, &paths)
}

fn add_paths(root: &Path, paths: &[u8]) -> Result<()> {
	let mut child = Command::new("git")
		.args([
			"--literal-pathspecs",
			"add",
			"-A",
			"--pathspec-from-file=-",
			"--pathspec-file-nul",
		])
		.current_dir(root)
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.with_context(|| format!("failed to run `git add -A`; {GIT_HELP}"))?;

	let write_result = child.stdin.take().map(|mut stdin| stdin.write_all(paths));

	let output = child
		.wait_with_output()
		.context("failed to wait for `git add -A`")?;

	ensure_success(output, "git add -A")?;

	if let Some(Err(error)) = write_result {
		return Err(error).context("failed to send paths to `git add -A`");
	}

	Ok(())
}

fn display_args<S: AsRef<OsStr>>(args: &[S]) -> String {
	args.iter()
		.map(|arg| arg.as_ref().to_string_lossy())
		.collect::<Vec<_>>()
		.join(" ")
}

fn ensure_success(output: Output, description: &str) -> Result<Output> {
	if !output.status.success() {
		let stderr = String::from_utf8_lossy(&output.stderr);
		let stderr = stderr.trim();
		if stderr.is_empty() {
			bail!("{description} failed with {}", output.status);
		}
		bail!("{description} failed: {stderr}");
	}

	Ok(output)
}

pub fn run_os(root: &Path, args: &[OsString]) -> Result<Output> {
	Command::new("git")
		.args(args)
		.current_dir(root)
		.output()
		.with_context(|| format!("failed to run `git {}`; {GIT_HELP}", display_args(args)))
}

pub fn run(root: &Path, args: &[&str]) -> Result<Output> {
	let args: Vec<OsString> = args.iter().copied().map(OsString::from).collect();
	run_os(root, &args)
}

pub fn run_checked(root: &Path, args: &[&str]) -> Result<Output> {
	let output = run(root, args)?;
	ensure_success(output, &format!("git {}", args.join(" ")))
}

pub fn run_os_checked(root: &Path, args: &[OsString]) -> Result<Output> {
	let output = run_os(root, args)?;
	ensure_success(output, &format!("git {}", display_args(args)))
}

pub fn check() -> Result<()> {
	let output = Command::new("git")
		.arg("--version")
		.output()
		.context(format!("failed to run `git --version`; {GIT_HELP}"))?;
	ensure_success(output, "git --version")?;
	Ok(())
}

pub fn init(root: &Path) -> Result<()> {
	run_checked(root, &["init", "-b", "main"])?;
	Ok(())
}

pub fn require_repo(root: &Path) -> Result<()> {
	let output = run(root, &["rev-parse", "--is-inside-work-tree"])?;
	if !output.status.success() {
		bail!(
			"{} is not a git repository; run `git init -b main` first",
			root.display()
		);
	}

	Ok(())
}

pub fn remotes(root: &Path) -> Result<Vec<String>> {
	let output = run_checked(root, &["remote"])?;
	Ok(String::from_utf8_lossy(&output.stdout)
		.lines()
		.map(str::trim)
		.filter(|line| !line.is_empty())
		.map(str::to_string)
		.collect())
}

pub fn remote_url(root: &Path, remote: &str) -> Result<String> {
	let output = run_checked(root, &["remote", "get-url", remote])?;
	Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn add_remote(root: &Path, name: &str, url: &str) -> Result<()> {
	run_checked(root, &["remote", "add", name, url])?;
	Ok(())
}

pub fn current_branch(root: &Path) -> Result<String> {
	let output = run_checked(root, &["symbolic-ref", "--quiet", "--short", "HEAD"])?;
	let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
	if branch.is_empty() {
		bail!("git: cannot determine the current branch");
	}

	Ok(branch)
}

pub fn has_head(root: &Path) -> Result<bool> {
	let output = run(root, &["rev-parse", "--verify", "--quiet", "HEAD"])?;
	Ok(output.status.success())
}

pub fn head_short(root: &Path) -> Result<String> {
	let output = run_checked(root, &["rev-parse", "--short", "HEAD"])?;
	Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn add(root: &Path, pathspecs: &[String]) -> Result<()> {
	let mut args = vec![
		OsString::from("add"),
		OsString::from("-A"),
		OsString::from("--"),
	];
	args.extend(pathspecs.iter().map(|spec| OsString::from(spec.as_str())));
	run_os_checked(root, &args)?;
	Ok(())
}

pub fn has_staged_changes(root: &Path, pathspecs: &[String]) -> Result<bool> {
	let mut args = vec![
		OsString::from("diff"),
		OsString::from("--cached"),
		OsString::from("--quiet"),
		OsString::from("--"),
	];
	args.extend(pathspecs.iter().map(|spec| OsString::from(spec.as_str())));

	let output = run_os(root, &args)?;
	match output.status.code() {
		Some(0) => Ok(false),
		Some(1) => Ok(true),
		_ => {
			ensure_success(output, "git diff --cached --quiet")?;
			Ok(false)
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedChange {
	pub status: String,
	pub path: String,
}

pub fn staged_changes(root: &Path, pathspecs: &[String]) -> Result<Vec<StagedChange>> {
	let mut args = vec![
		OsString::from("diff"),
		OsString::from("--cached"),
		OsString::from("--name-status"),
		OsString::from("--"),
	];
	args.extend(pathspecs.iter().map(|spec| OsString::from(spec.as_str())));

	let output = run_os_checked(root, &args)?;
	let mut changes = Vec::new();
	for line in String::from_utf8_lossy(&output.stdout).lines() {
		let mut parts = line.splitn(3, '\t');
		let status = parts.next().unwrap_or_default();
		let first = parts.next().unwrap_or_default();
		let path = match parts.next() {
			Some(second) => format!("{first} -> {second}"),
			None => first.to_string(),
		};

		if !status.is_empty() && !path.is_empty() {
			changes.push(StagedChange {
				status: status.to_string(),
				path,
			});
		}
	}

	Ok(changes)
}

pub fn commit(root: &Path, message: &str, pathspecs: &[String]) -> Result<()> {
	let mut args = vec![
		OsString::from("commit"),
		OsString::from("-m"),
		OsString::from(message),
		OsString::from("--only"),
		OsString::from("--"),
	];
	args.extend(pathspecs.iter().map(|spec| OsString::from(spec.as_str())));

	let output = run_os(root, &args)?;
	if output.status.success() {
		return Ok(());
	}

	let stderr = String::from_utf8_lossy(&output.stderr);
	if stderr.contains("Please tell me who you are")
		|| stderr.contains("unable to auto-detect email address")
	{
		bail!(
			"git commit failed because user.name and user.email are not configured; run `git config user.name \"Your Name\"` and `git config user.email \"you@example.com\"`"
		);
	}

	ensure_success(output, &format!("git {}", display_args(&args)))?;
	Ok(())
}

pub fn has_upstream(root: &Path) -> Result<bool> {
	let output = run(
		root,
		&[
			"rev-parse",
			"--abbrev-ref",
			"--symbolic-full-name",
			"@{upstream}",
		],
	)?;
	Ok(output.status.success())
}

pub fn push(root: &Path, remote: &str, branch: &str, set_upstream: bool) -> Result<()> {
	if set_upstream {
		run_checked(root, &["push", "--set-upstream", remote, branch])?;
	} else {
		run_checked(root, &["push"])?;
	}

	Ok(())
}
