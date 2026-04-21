import fs from "node:fs";
import fsp from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import { stdin, stdout, stderr, exit } from "node:process";

import { simpleGit } from "simple-git";

const FILE_CHANGE_TEXT_SNAPSHOT_BYTE_LIMIT = 256 * 1024;
const REVIEW_DIFF_CHAR_LIMIT = 120_000;
const REVIEW_UNTRACKED_FILE_LIMIT = 5;
const REVIEW_UNTRACKED_FILE_SIZE_LIMIT = 16 * 1024;
const REVIEW_UNTRACKED_TOTAL_CHAR_LIMIT = 48_000;
const COMMIT_DETAIL_DIFF_CHAR_LIMIT = 120_000;

async function readInput() {
  let raw = "";
  for await (const chunk of stdin) {
    raw += chunk.toString();
  }
  if (!raw.trim()) {
    throw new Error("missing input payload");
  }
  return JSON.parse(raw);
}

function emit(payload) {
  stdout.write(`${JSON.stringify(payload)}\n`);
}

function emitError(error) {
  const message = error instanceof Error ? error.message : String(error ?? "unknown error");
  emit({ ok: false, error: message });
}

function expandHome(value) {
  if (typeof value !== "string") {
    throw new Error("repoPath 必须是字符串");
  }
  const trimmed = value.trim();
  if (!trimmed) {
    throw new Error("repoPath 不能为空");
  }
  if (trimmed === "~") {
    return os.homedir();
  }
  if (trimmed.startsWith("~/")) {
    return path.join(os.homedir(), trimmed.slice(2));
  }
  return trimmed;
}

function resolveRepoPath(value) {
  return path.normalize(expandHome(value));
}

function resolveTargetPath(repoPath, candidatePath) {
  const normalized = typeof candidatePath === "string" ? candidatePath.trim() : "";
  if (!normalized) {
    throw new Error("路径不能为空");
  }
  const expanded = expandHome(normalized);
  return path.isAbsolute(expanded) ? expanded : path.join(repoPath, expanded);
}

async function gitCommonDir(repoPath) {
  const output = (await gitRaw(repoPath, ["rev-parse", "--git-common-dir"])).trim();
  if (!output) {
    throw new Error("无法解析 Git 公共目录");
  }
  return path.normalize(path.isAbsolute(output) ? output : path.join(repoPath, output));
}

function buildGit(repoPath) {
  return simpleGit({
    baseDir: repoPath,
    binary: "git",
    maxConcurrentProcesses: 1,
    trimmed: false,
  });
}

async function ensureGitRepository(repoPath) {
  const gitMarker = path.join(repoPath, ".git");
  if (!fs.existsSync(gitMarker)) {
    throw new Error(`工作目录 ${repoPath} 不是 Git 仓库，缺少 .git`);
  }
  const git = buildGit(repoPath);
  try {
    await git.revparse(["--git-dir"]);
  } catch (error) {
    throw new Error(error instanceof Error ? error.message : String(error));
  }
}

async function gitRaw(repoPath, args) {
  return buildGit(repoPath).raw(args);
}

async function gitCli(repoPath, args) {
  return await new Promise((resolve, reject) => {
    const child = spawn("git", args, {
      cwd: repoPath,
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdoutBuffer = "";
    let stderrBuffer = "";

    child.stdout.on("data", (chunk) => {
      stdoutBuffer += chunk.toString();
    });
    child.stderr.on("data", (chunk) => {
      stderrBuffer += chunk.toString();
    });
    child.on("error", (error) => {
      reject(error);
    });
    child.on("close", (code) => {
      if (code === 0) {
        resolve(stdoutBuffer);
        return;
      }
      const message = stderrBuffer.trim() || stdoutBuffer.trim() || `git ${args.join(" ")} 执行失败`;
      reject(new Error(message));
    });
  });
}

function optionalText(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function requiredText(input, keys, label) {
  for (const key of keys) {
    const normalized = optionalText(input?.[key]);
    if (normalized) {
      return normalized;
    }
  }
  throw new Error(`${label} 不能为空`);
}

async function gitRefExists(repoPath, fullRef) {
  try {
    await gitCli(repoPath, ["show-ref", "--verify", "--quiet", fullRef]);
    return true;
  } catch {
    return false;
  }
}

async function determineDefaultBranch(repoPath) {
  try {
    const value = (await gitRaw(repoPath, ["symbolic-ref", "refs/remotes/origin/HEAD", "--short"])).trim();
    const branch = value.split("/").pop()?.trim();
    if (branch) {
      return branch;
    }
  } catch {}

  try {
    const branch = (await gitRaw(repoPath, ["rev-parse", "--abbrev-ref", "HEAD"])).trim();
    if (branch && branch !== "HEAD") {
      return branch;
    }
  } catch {}

  if (await gitRefExists(repoPath, "refs/heads/main")) {
    return "main";
  }
  if (await gitRefExists(repoPath, "refs/heads/master")) {
    return "master";
  }
  throw new Error("无法解析默认目标分支");
}

async function currentBranch(repoPath) {
  const value = (await gitRaw(repoPath, ["rev-parse", "--abbrev-ref", "HEAD"])).trim();
  if (!value || value === "HEAD") {
    return null;
  }
  return value;
}

async function isWorkingTreeClean(repoPath) {
  return (await gitRaw(repoPath, ["status", "--porcelain"])).trim().length === 0;
}

async function mergeTaskBranchIntoTarget(repoPath, taskBranch, targetBranch, strategy, allowFF) {
  const branch = await currentBranch(repoPath);
  if (branch !== targetBranch) {
    if (!(await isWorkingTreeClean(repoPath))) {
      throw new Error(
        `项目主工作区当前在 ${branch ?? "detached HEAD"}，且存在未提交改动，无法切换到目标分支 ${targetBranch} 执行合并`,
      );
    }
    await gitCli(repoPath, ["checkout", targetBranch]);
  }

  const args = ["merge"];
  if (allowFF === false) {
    args.push("--no-ff");
  }
  args.push(`--strategy=${strategy ?? "ort"}`);
  args.push(taskBranch);
  await gitCli(repoPath, args);
  return `已将任务分支 ${taskBranch} 合并到目标分支 ${targetBranch}`;
}

async function resolveSyncTargetRef(repoPath, branchName) {
  if (!branchName) {
    return null;
  }

  try {
    const upstream = (
      await gitRaw(repoPath, ["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"])
    ).trim();
    if (upstream) {
      return upstream;
    }
  } catch {}

  const originRef = `origin/${branchName}`;
  if (await gitRefExists(repoPath, `refs/remotes/${originRef}`)) {
    return originRef;
  }

  return null;
}

async function branchSyncCounts(repoPath, branchName) {
  const syncTargetRef = await resolveSyncTargetRef(repoPath, branchName);
  if (!syncTargetRef) {
    return {
      ahead_commits: null,
      behind_commits: null,
    };
  }

  const output = (
    await gitRaw(repoPath, ["rev-list", "--left-right", "--count", `${syncTargetRef}...HEAD`])
  ).trim();
  const [behindRaw = "", aheadRaw = ""] = output.split(/\s+/);
  const behindCommits = Number.parseInt(behindRaw, 10);
  const aheadCommits = Number.parseInt(aheadRaw, 10);

  return {
    ahead_commits: Number.isFinite(aheadCommits) ? aheadCommits : null,
    behind_commits: Number.isFinite(behindCommits) ? behindCommits : null,
  };
}

async function compareRevisions(repoPath, baseRevision, targetRevision) {
  const output = (
    await gitRaw(repoPath, ["rev-list", "--left-right", "--count", `${baseRevision}...${targetRevision}`])
  ).trim();
  const [behindRaw = "", aheadRaw = ""] = output.split(/\s+/);
  const behindCommits = Number.parseInt(behindRaw, 10);
  const aheadCommits = Number.parseInt(aheadRaw, 10);

  if (!Number.isFinite(behindCommits) || !Number.isFinite(aheadCommits)) {
    throw new Error(`无法解析 revision 比较结果: ${output}`);
  }

  return {
    ahead_commits: aheadCommits,
    behind_commits: behindCommits,
  };
}

async function listBranches(repoPath) {
  const summary = await buildGit(repoPath).branchLocal();
  return [...summary.all].sort((left, right) => left.localeCompare(right));
}

async function headCommit(repoPath, revision = "HEAD") {
  return (await gitRaw(repoPath, ["rev-parse", revision])).trim();
}

function summarizeWorkingTreeFromStatus(statusOutput) {
  const status = statusOutput.trim();
  if (!status) {
    return null;
  }

  let modified = 0;
  let added = 0;
  let deleted = 0;
  let renamed = 0;
  let untracked = 0;
  let total = 0;

  for (const line of status.split(/\r?\n/)) {
    const code = line.slice(0, 2).trim();
    if (!code) {
      continue;
    }
    total += 1;
    if (code === "??") {
      untracked += 1;
      continue;
    }
    if (code.includes("M")) modified += 1;
    if (code.includes("A")) added += 1;
    if (code.includes("D")) deleted += 1;
    if (code.includes("R")) renamed += 1;
  }

  const parts = [];
  if (modified > 0) parts.push(`修改 ${modified}`);
  if (added > 0) parts.push(`新增 ${added}`);
  if (deleted > 0) parts.push(`删除 ${deleted}`);
  if (renamed > 0) parts.push(`重命名 ${renamed}`);
  if (untracked > 0) parts.push(`未跟踪 ${untracked}`);
  return `共 ${total} 项变更（${parts.join("，")}）`;
}

function normalizeCommitChangeType(statusCode) {
  const normalized = typeof statusCode === "string" ? statusCode.trim().toUpperCase() : "";
  if (normalized.startsWith("A")) {
    return "added";
  }
  if (normalized.startsWith("D")) {
    return "deleted";
  }
  if (normalized.startsWith("R")) {
    return "renamed";
  }
  return "modified";
}

function parseCommitFileChange(line) {
  const trimmed = typeof line === "string" ? line.trim() : "";
  if (!trimmed) {
    return null;
  }

  const [statusCode = "", ...paths] = trimmed.split("\t");
  if (!statusCode || paths.length === 0) {
    return null;
  }

  const changeType = normalizeCommitChangeType(statusCode);
  if (changeType === "renamed") {
    const [previousPath = "", nextPath = ""] = paths;
    return {
      path: nextPath.trim() || previousPath.trim(),
      previous_path: previousPath.trim() || null,
      change_type: changeType,
    };
  }

  return {
    path: paths[0]?.trim() ?? "",
    previous_path: null,
    change_type: changeType,
  };
}

async function listCommitHistory(repoPath, offset = 0, limit = 20) {
  const normalizedOffset = Number.isFinite(offset) ? Math.max(0, Math.floor(offset)) : 0;
  const normalizedLimit = Number.isFinite(limit) ? Math.max(1, Math.floor(limit)) : 20;
  const output = (
    await gitRaw(repoPath, [
      "log",
      "--format=%H%x1f%h%x1f%s%x1f%an%x1f%ad",
      "--date=format:%Y-%m-%d %H:%M:%S",
      `--skip=${normalizedOffset}`,
      "-n",
      String(normalizedLimit + 1),
    ])
  ).trim();

  if (!output) {
    return {
      commits: [],
      has_more: false,
    };
  }

  const lines = output.split(/\r?\n/);
  const hasMore = lines.length > normalizedLimit;
  const commits = lines.slice(0, normalizedLimit).map((line) => {
    const [sha = "", shortSha = "", subject = "", authorName = "", authoredAt = ""] = line.split("\u001f");
    return {
      sha: sha.trim(),
      short_sha: shortSha.trim(),
      subject: subject.trim(),
      author_name: authorName.trim(),
      authored_at: authoredAt.trim(),
    };
  });

  return {
    commits,
    has_more: hasMore,
  };
}

async function getCommitDetail(repoPath, commitRef) {
  const normalizedCommitRef = optionalText(commitRef);
  if (!normalizedCommitRef) {
    throw new Error("commitSha 不能为空");
  }

  const metadataOutput = (
    await gitRaw(repoPath, [
      "log",
      "-1",
      "--format=%H%x1f%h%x1f%s%x1f%an%x1f%ae%x1f%ad%x1f%b",
      "--date=format:%Y-%m-%d %H:%M:%S",
      normalizedCommitRef,
    ])
  ).trimEnd();
  if (!metadataOutput.trim()) {
    throw new Error(`未找到提交 ${normalizedCommitRef}`);
  }

  const [sha = "", shortSha = "", subject = "", authorName = "", authorEmail = "", authoredAt = "", ...bodyParts] =
    metadataOutput.split("\u001f");
  const changedFilesOutput = (
    await gitRaw(repoPath, ["show", "--format=", "--name-status", "--find-renames", normalizedCommitRef])
  ).trim();
  const rawDiffText = (
    await gitRaw(repoPath, ["show", "--format=", "--no-ext-diff", normalizedCommitRef])
  ).trimEnd();
  const diffTruncated = rawDiffText.length > COMMIT_DETAIL_DIFF_CHAR_LIMIT;
  const diffText = rawDiffText.slice(0, COMMIT_DETAIL_DIFF_CHAR_LIMIT).trim();

  return {
    sha: sha.trim(),
    short_sha: shortSha.trim(),
    subject: subject.trim(),
    body: optionalText(bodyParts.join("\u001f")),
    author_name: authorName.trim(),
    author_email: optionalText(authorEmail),
    authored_at: authoredAt.trim(),
    diff_text: diffText || null,
    diff_truncated: diffTruncated,
    changed_files: changedFilesOutput
      ? changedFilesOutput
        .split(/\r?\n/)
        .map(parseCommitFileChange)
        .filter((item) => item !== null)
      : [],
  };
}

function captureTextSnapshotFromBuffer(buffer, truncatedHint) {
  if (buffer.includes(0)) {
    return {
      status: "binary",
      text: null,
      truncated: false,
    };
  }

  return {
    status: "text",
    text: buffer.toString("utf8"),
    truncated: Boolean(truncatedHint),
  };
}

async function captureWorktreeTextSnapshot(repoPath, relativePath) {
  const targetPath = resolveTargetPath(repoPath, relativePath);
  let stat;
  try {
    stat = await fsp.stat(targetPath);
  } catch {
    return {
      status: "missing",
      text: null,
      truncated: false,
    };
  }

  if (!stat.isFile()) {
    return {
      status: "unavailable",
      text: null,
      truncated: false,
    };
  }

  const handle = await fsp.open(targetPath, "r");
  try {
    const byteLimit = FILE_CHANGE_TEXT_SNAPSHOT_BYTE_LIMIT + 4;
    const buffer = Buffer.alloc(Number(Math.min(stat.size, byteLimit)));
    const { bytesRead } = await handle.read(buffer, 0, buffer.length, 0);
    return captureTextSnapshotFromBuffer(
      buffer.subarray(0, bytesRead),
      stat.size > FILE_CHANGE_TEXT_SNAPSHOT_BYTE_LIMIT,
    );
  } finally {
    await handle.close();
  }
}

async function captureRevisionTextSnapshot(repoPath, revision, relativePath) {
  const normalizedRevision = optionalText(revision);
  if (!normalizedRevision) {
    throw new Error("revision 不能为空");
  }

  try {
    const output = await gitRaw(repoPath, ["show", `${normalizedRevision}:${relativePath}`]);
    return captureTextSnapshotFromBuffer(
      Buffer.from(output, "utf8"),
      output.length > FILE_CHANGE_TEXT_SNAPSHOT_BYTE_LIMIT,
    );
  } catch {
    return {
      status: "missing",
      text: null,
      truncated: false,
    };
  }
}

async function captureHeadTextSnapshot(repoPath, relativePath) {
  return captureRevisionTextSnapshot(repoPath, "HEAD", relativePath);
}

function shouldReadPreviousPath(statusX, statusY) {
  return ["R", "C"].includes(statusX) || ["R", "C"].includes(statusY);
}

function parseStatusEntries(statusOutput) {
  const parts = statusOutput.split("\0");
  const entries = [];
  let index = 0;

  while (index < parts.length) {
    const segment = parts[index];
    index += 1;

    if (!segment) {
      continue;
    }

    if (segment.length < 4) {
      throw new Error(`无法解析 git status 输出片段: ${segment}`);
    }

    const statusX = segment[0];
    const statusY = segment[1];
    const filePath = segment.slice(3);
    let previousPath = null;
    if (shouldReadPreviousPath(statusX, statusY)) {
      previousPath = parts[index] || null;
      if (!previousPath) {
        throw new Error(`git status 缺少重命名原路径: ${filePath}`);
      }
      index += 1;
    }

    entries.push({
      path: filePath,
      previous_path: previousPath,
      status_x: statusX,
      status_y: statusY,
    });
  }

  return entries;
}

function classifyStatusEntry(entry) {
  const { status_x: statusX, status_y: statusY } = entry;
  if (statusX === "R" || statusY === "R") {
    return "renamed";
  }
  if (statusX === "D" || statusY === "D") {
    return "deleted";
  }
  if (["A", "?"].includes(statusX) || ["A", "?"].includes(statusY)) {
    return "added";
  }
  return "modified";
}

function deriveStageStatus(entry) {
  const { status_x: statusX, status_y: statusY } = entry;
  if (statusX === "?" && statusY === "?") {
    return "untracked";
  }
  const staged = statusX !== " " && statusX !== "?";
  const unstaged = statusY !== " " && statusY !== "?";
  if (staged && unstaged) {
    return "partially_staged";
  }
  if (staged) {
    return "staged";
  }
  if (unstaged) {
    return "unstaged";
  }
  return "unstaged";
}

function normalizeGitPathArg(repoPath, candidatePath) {
  const resolved = resolveTargetPath(repoPath, candidatePath);
  const relative = path.relative(repoPath, resolved);
  return relative || ".";
}

async function hasHeadCommit(repoPath) {
  try {
    await gitRaw(repoPath, ["rev-parse", "--verify", "HEAD"]);
    return true;
  } catch {
    return false;
  }
}

async function stagePath(repoPath, targetPath) {
  await gitRaw(repoPath, ["add", "--", normalizeGitPathArg(repoPath, targetPath)]);
}

async function stageAll(repoPath) {
  await gitRaw(repoPath, ["add", "-A"]);
}

async function unstagePath(repoPath, targetPath) {
  const normalizedPath = normalizeGitPathArg(repoPath, targetPath);
  if (await hasHeadCommit(repoPath)) {
    try {
      await gitRaw(repoPath, ["reset", "HEAD", "--", normalizedPath]);
      return;
    } catch {
      // unborn HEAD 或 reset 不可用时继续 fallback
    }
  }
  await gitRaw(repoPath, ["rm", "--cached", "-r", "--ignore-unmatch", "--", normalizedPath]);
}

async function unstageAll(repoPath) {
  if (await hasHeadCommit(repoPath)) {
    try {
      await gitRaw(repoPath, ["reset", "HEAD", "--", "."]);
      return;
    } catch {
      // unborn HEAD 或 reset 不可用时继续 fallback
    }
  }
  await gitRaw(repoPath, ["rm", "--cached", "-r", "--ignore-unmatch", "--", "."]);
}

async function hasStagedChanges(repoPath) {
  const statusOutput = await gitRaw(repoPath, ["status", "--porcelain=v1", "-z", "--untracked-files=all"]);
  return parseStatusEntries(statusOutput).some((entry) => {
    const stageStatus = deriveStageStatus(entry);
    return stageStatus === "staged" || stageStatus === "partially_staged";
  });
}

async function commitChanges(repoPath, message) {
  const trimmed = typeof message === "string" ? message.trim() : "";
  if (!trimmed) {
    throw new Error("提交说明不能为空");
  }
  if (!(await hasStagedChanges(repoPath))) {
    throw new Error("当前没有已暂存的改动，无法创建提交");
  }
  await gitRaw(repoPath, ["commit", "-m", trimmed]);
  const head = (await gitRaw(repoPath, ["log", "-1", "--format=%h %s"])).trim();
  return head ? `已创建提交 ${head}` : "已创建提交";
}

async function pushBranch(repoPath, remoteName, branchName, forceMode) {
  const remote = typeof remoteName === "string" && remoteName.trim() ? remoteName.trim() : "origin";
  const branch = typeof branchName === "string" && branchName.trim() ? branchName.trim() : await currentBranch(repoPath);
  if (!branch) {
    throw new Error("无法解析当前分支，不能推送");
  }

  const args = ["push"];
  if (forceMode === "force") {
    args.push("--force");
  } else if (forceMode === "force_with_lease") {
    args.push("--force-with-lease");
  }
  args.push(remote, branch);
  await gitRaw(repoPath, args);
  return `已推送 ${branch} 到 ${remote}`;
}

async function pullBranch(repoPath, remoteName, branchName, mode, autoStash) {
  const remote = typeof remoteName === "string" && remoteName.trim() ? remoteName.trim() : "origin";
  const branch = typeof branchName === "string" && branchName.trim() ? branchName.trim() : await currentBranch(repoPath);
  if (!branch) {
    throw new Error("无法解析当前分支，不能拉取");
  }

  const args = ["pull"];
  if (mode === "rebase" && autoStash) {
    args.push("--autostash");
  }
  if (mode === "rebase") {
    args.push("--rebase");
  } else {
    args.push("--ff-only");
  }
  args.push(remote, branch);
  await gitRaw(repoPath, args);
  return mode === "rebase"
    ? `已通过 rebase 拉取 ${remote}/${branch}`
    : `已拉取 ${remote}/${branch}`;
}

async function hashWorktreePath(repoPath, relativePath) {
  const targetPath = resolveTargetPath(repoPath, relativePath);
  if (!fs.existsSync(targetPath)) {
    return null;
  }

  const gitPath = path.isAbsolute(relativePath) ? targetPath : relativePath;
  const output = await gitRaw(repoPath, ["hash-object", "--no-filters", "--", gitPath]);
  const hash = output.trim();
  return hash || null;
}

async function collectSnapshot(repoPath, captureTextSnapshots) {
  const statusOutput = await gitRaw(repoPath, ["status", "--porcelain=v1", "-z", "--untracked-files=all"]);
  const parsedEntries = parseStatusEntries(statusOutput);
  const entries = [];

  for (const entry of parsedEntries) {
    const contentHash = await hashWorktreePath(repoPath, entry.path);
    const textSnapshot = captureTextSnapshots
      ? await captureWorktreeTextSnapshot(repoPath, entry.path)
      : { status: "missing", text: null, truncated: false };
    entries.push({
      ...entry,
      content_hash: contentHash,
      text_snapshot: textSnapshot,
    });
  }

  return entries;
}

async function buildReviewUntrackedSnippets(repoPath, untrackedFiles) {
  const snippets = [];
  let consumedChars = 0;

  for (const relativePath of untrackedFiles.slice(0, REVIEW_UNTRACKED_FILE_LIMIT)) {
    if (consumedChars >= REVIEW_UNTRACKED_TOTAL_CHAR_LIMIT) {
      break;
    }

    const targetPath = resolveTargetPath(repoPath, relativePath);
    let stat;
    try {
      stat = await fsp.stat(targetPath);
    } catch {
      continue;
    }

    if (!stat.isFile() || stat.size > REVIEW_UNTRACKED_FILE_SIZE_LIMIT) {
      continue;
    }

    let content;
    try {
      content = await fsp.readFile(targetPath, "utf8");
    } catch {
      continue;
    }

    const remaining = REVIEW_UNTRACKED_TOTAL_CHAR_LIMIT - consumedChars;
    if (remaining <= 0) {
      break;
    }
    const snippet = content.slice(0, Math.min(remaining, 12_000));
    if (!snippet) {
      continue;
    }

    consumedChars += snippet.length;
    snippets.push(`### ${relativePath}\n\`\`\`text\n${snippet}\n\`\`\`\n${snippet.length < content.length ? "（内容已截断）" : ""}`);
  }

  if (snippets.length === 0) {
    return "（无可直接读取的未跟踪文本文件内容）";
  }

  return snippets.join("\n\n");
}

function buildUntrackedReviewSection(untrackedFiles, snippets) {
  if (untrackedFiles.length === 0) {
    return "（无未跟踪文件）";
  }

  return `未跟踪文件列表：\n${untrackedFiles.map((item) => `- ${item}`).join("\n")}\n\n未跟踪文本文件摘录：\n${snippets}`;
}

function buildReviewContextFromGitOutputs(
  statusOutput,
  unstagedStat,
  unstagedDiff,
  stagedStat,
  stagedDiff,
  untrackedFiles,
  untrackedSection,
) {
  const statusTrimmed = statusOutput.trim();
  if (!statusTrimmed) {
    throw new Error("当前工作区没有可审核的代码改动");
  }

  const combinedDiff = [stagedDiff.trim(), unstagedDiff.trim()].filter(Boolean).join("\n\n");
  if (!combinedDiff.trim() && untrackedFiles.length === 0) {
    throw new Error("当前工作区没有可审核的代码 diff");
  }

  const combinedStat = [stagedStat.trim(), unstagedStat.trim()].filter(Boolean).join("\n");
  const diffBody = combinedDiff.slice(0, REVIEW_DIFF_CHAR_LIMIT);
  const diffTruncated = combinedDiff.length > REVIEW_DIFF_CHAR_LIMIT;

  return `## Git 状态\n${statusTrimmed}\n\n## Diff 概览\n${
    combinedStat.trim() ? combinedStat.trim() : "（无 diff 统计）"
  }\n\n## 完整 Diff\n${
    diffBody.trim() ? diffBody.trim() : "（无已跟踪文件 diff）"
  }\n${diffTruncated ? "\n（完整 diff 已截断）" : ""}\n\n## 未跟踪文件\n${untrackedSection}`;
}

async function collectReviewContext(repoPath) {
  const statusOutput = await gitRaw(repoPath, ["status", "--short"]);
  const unstagedStat = await gitRaw(repoPath, ["diff", "--no-ext-diff", "--stat"]);
  const unstagedDiff = await gitRaw(repoPath, ["diff", "--no-ext-diff"]);
  const stagedStat = await gitRaw(repoPath, ["diff", "--no-ext-diff", "--stat", "--cached"]);
  const stagedDiff = await gitRaw(repoPath, ["diff", "--no-ext-diff", "--cached"]);
  const untrackedOutput = await gitRaw(repoPath, ["ls-files", "--others", "--exclude-standard"]);
  const untrackedFiles = untrackedOutput
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
  const untrackedSection = buildUntrackedReviewSection(
    untrackedFiles,
    await buildReviewUntrackedSnippets(repoPath, untrackedFiles),
  );

  return buildReviewContextFromGitOutputs(
    statusOutput,
    unstagedStat,
    unstagedDiff,
    stagedStat,
    stagedDiff,
    untrackedFiles,
    untrackedSection,
  );
}

async function ensureTaskBranch(repoPath, taskBranch, targetBranch) {
  const normalizedTaskBranch = optionalText(taskBranch);
  const normalizedTargetBranch = optionalText(targetBranch);
  if (!normalizedTaskBranch) {
    throw new Error("taskBranch 不能为空");
  }
  if (!normalizedTargetBranch) {
    throw new Error("targetBranch 不能为空");
  }
  if (await gitRefExists(repoPath, `refs/heads/${normalizedTaskBranch}`)) {
    return;
  }
  await gitCli(repoPath, ["branch", normalizedTaskBranch, normalizedTargetBranch]);
}

async function ensureTaskWorktree(repoPath, worktreePath, taskBranch, targetBranch) {
  const resolvedWorktreePath = resolveTargetPath(repoPath, worktreePath);
  const normalizedTaskBranch = optionalText(taskBranch);
  const normalizedTargetBranch = optionalText(targetBranch);
  if (!normalizedTaskBranch) {
    throw new Error("taskBranch 不能为空");
  }
  if (fs.existsSync(path.join(resolvedWorktreePath, ".git"))) {
    return;
  }

  if (fs.existsSync(resolvedWorktreePath)) {
    const dirEntries = await fsp.readdir(resolvedWorktreePath);
    if (dirEntries.length > 0) {
      throw new Error(`worktree 目录已存在且非空：${resolvedWorktreePath}`);
    }
  } else {
    await fsp.mkdir(path.dirname(resolvedWorktreePath), { recursive: true });
  }

  if (await gitRefExists(repoPath, `refs/heads/${normalizedTaskBranch}`)) {
    await gitCli(repoPath, ["worktree", "add", resolvedWorktreePath, normalizedTaskBranch]);
    return;
  }
  if (!normalizedTargetBranch) {
    throw new Error("targetBranch 不能为空");
  }
  await gitCli(repoPath, [
    "worktree",
    "add",
    "-b",
    normalizedTaskBranch,
    resolvedWorktreePath,
    normalizedTargetBranch,
  ]);
}

async function executeAction(repoPath, worktreePath, taskBranch, actionType, payload) {
  const resolvedWorktreePath = resolveTargetPath(repoPath, worktreePath);
  let worktreeGit = null;
  const getWorktreeGit = () => {
    if (!worktreeGit) {
      worktreeGit = buildGit(resolvedWorktreePath);
    }
    return worktreeGit;
  };

  switch (actionType) {
    case "merge": {
      const targetBranch = optionalText(payload.target_branch) ?? optionalText(payload.targetBranch);
      if (!targetBranch) {
        throw new Error("merge 缺少 target_branch");
      }
      return mergeTaskBranchIntoTarget(
        repoPath,
        taskBranch,
        targetBranch,
        payload.strategy ?? "ort",
        payload.allow_ff,
      );
    }
    case "push": {
      const args = ["push"];
      if (payload.force_mode === "force") {
        args.push("--force");
      } else if (payload.force_mode === "force_with_lease") {
        args.push("--force-with-lease");
      }
      args.push(payload.remote_name);
      args.push(`${payload.source_branch}:${payload.target_ref}`);
      await getWorktreeGit().raw(args);
      return `已推送 ${payload.source_branch} 到 ${payload.target_ref}`;
    }
    case "rebase": {
      const args = ["rebase"];
      if (payload.auto_stash) {
        args.push("--autostash");
      }
      args.push(payload.onto_branch);
      await getWorktreeGit().raw(args);
      return `已将任务分支 rebase 到 ${payload.onto_branch}`;
    }
    case "cherry_pick": {
      await getWorktreeGit().raw(["cherry-pick", ...payload.commit_ids]);
      return "已完成 cherry-pick";
    }
    case "stash": {
      const args = ["stash", "push"];
      if (payload.include_untracked) {
        args.push("--include-untracked");
      }
      if (payload.message) {
        args.push("-m", payload.message);
      }
      await getWorktreeGit().raw(args);
      return "已创建 stash";
    }
    case "unstash": {
      const stashRef = payload.stash_ref || "stash@{0}";
      await getWorktreeGit().raw(["stash", "pop", stashRef]);
      return `已恢复 ${stashRef}`;
    }
    case "cleanup_worktree": {
      const forceRemove = payload.force_remove !== false;
      const worktreeRegistered = fs.existsSync(path.join(resolvedWorktreePath, ".git"));
      try {
        const args = ["worktree", "remove", resolvedWorktreePath];
        if (forceRemove) {
          args.push("--force");
        }
        await gitRaw(repoPath, args);
      } catch (error) {
        if (worktreeRegistered) {
          throw error;
        }
        // drifted worktree 允许继续做兜底清理
      }
      if (payload.delete_branch && await gitRefExists(repoPath, `refs/heads/${taskBranch}`)) {
        await gitRaw(repoPath, ["branch", "-D", taskBranch]);
      }
      if (payload.prune_worktree !== false) {
        await gitRaw(repoPath, ["worktree", "prune"]);
      }
      await fsp.rm(resolvedWorktreePath, { recursive: true, force: true });
      return "已清理任务 worktree";
    }
    default:
      throw new Error(`不支持的 git action: ${actionType}`);
  }
}

async function executeCommand(input) {
  const repoPath = input.repoPath ? resolveRepoPath(input.repoPath) : null;
  if (repoPath) {
    await ensureGitRepository(repoPath);
  }

  switch (input.command) {
    case "overview": {
      const statusOutput = await gitRaw(repoPath, ["status", "--short"]);
      const branchName = await currentBranch(repoPath);
      const syncCounts = await branchSyncCounts(repoPath, branchName);
      const recentCommitHistory = await listCommitHistory(repoPath, 0, Number(input.recentCommitLimit ?? 5));
      return {
        default_branch: await determineDefaultBranch(repoPath),
        current_branch: branchName,
        project_branches: await listBranches(repoPath),
        head_commit_sha: await headCommit(repoPath, "HEAD"),
        working_tree_summary: summarizeWorkingTreeFromStatus(statusOutput),
        ahead_commits: syncCounts.ahead_commits,
        behind_commits: syncCounts.behind_commits,
        recent_commits: recentCommitHistory.commits,
        recent_commits_has_more: recentCommitHistory.has_more,
      };
    }
    case "commit_history":
      return await listCommitHistory(repoPath, Number(input.offset ?? 0), Number(input.limit ?? 20));
    case "commit_detail":
      return await getCommitDetail(repoPath, input.commitSha);
    case "path_exists":
      return { exists: fs.existsSync(resolveTargetPath(repoPath, input.targetPath)) };
    case "git_common_dir":
      return { path: await gitCommonDir(repoPath) };
    case "ref_exists":
      return { exists: await gitRefExists(repoPath, input.fullRef) };
    case "stage_path":
      await stagePath(repoPath, input.targetPath);
      return { message: "已暂存文件" };
    case "unstage_path":
      await unstagePath(repoPath, input.targetPath);
      return { message: "已取消暂存文件" };
    case "stage_all":
      await stageAll(repoPath);
      return { message: "已暂存全部文件" };
    case "unstage_all":
      await unstageAll(repoPath);
      return { message: "已取消暂存全部文件" };
    case "commit_changes":
      return { message: await commitChanges(repoPath, input.message) };
    case "push_branch":
      return {
        message: await pushBranch(repoPath, input.remoteName, input.branchName, input.forceMode),
      };
    case "pull_branch":
      return {
        message: await pullBranch(
          repoPath,
          input.remoteName,
          input.branchName,
          input.mode,
          Boolean(input.autoStash),
        ),
      };
    case "ensure_task_branch":
      await ensureTaskBranch(
        repoPath,
        requiredText(input, ["taskBranch", "task_branch"], "taskBranch"),
        requiredText(input, ["targetBranch", "target_branch"], "targetBranch"),
      );
      return { ok: true };
    case "ensure_task_worktree":
      await ensureTaskWorktree(
        repoPath,
        requiredText(input, ["worktreePath", "worktree_path"], "worktreePath"),
        requiredText(input, ["taskBranch", "task_branch"], "taskBranch"),
        optionalText(input.targetBranch) ?? optionalText(input.target_branch),
      );
      return { ok: true };
    case "rev_parse":
      return { sha: await headCommit(repoPath, input.revision || "HEAD") };
    case "compare_revisions":
      return await compareRevisions(
        repoPath,
        requiredText(input, ["baseRevision", "base_revision"], "baseRevision"),
        requiredText(input, ["targetRevision", "target_revision"], "targetRevision"),
      );
    case "execute_action":
      return {
        message: await executeAction(
          repoPath,
          input.worktreePath,
          input.taskBranch,
          input.actionType,
          input.payload ?? {},
        ),
      };
    case "collect_review_context":
      return { context: await collectReviewContext(repoPath) };
    case "status_changes": {
      const statusOutput = await gitRaw(repoPath, ["status", "--porcelain=v1", "-z", "--untracked-files=all"]);
      return {
        changes: parseStatusEntries(statusOutput).map((entry) => ({
          path: entry.path,
          previous_path: entry.previous_path,
          change_type: classifyStatusEntry(entry),
          stage_status: deriveStageStatus(entry),
        })),
      };
    }
    case "collect_snapshot":
      return {
        entries: await collectSnapshot(repoPath, Boolean(input.captureTextSnapshots)),
      };
    case "hash_worktree_path":
      return {
        content_hash: await hashWorktreePath(repoPath, input.relativePath),
      };
    case "capture_worktree_text_snapshot":
      return {
        snapshot: await captureWorktreeTextSnapshot(repoPath, input.relativePath),
      };
    case "capture_head_text_snapshot":
      return {
        snapshot: await captureHeadTextSnapshot(repoPath, input.relativePath),
      };
    case "capture_revision_text_snapshot":
      return {
        snapshot: await captureRevisionTextSnapshot(repoPath, input.revision, input.relativePath),
      };
    default:
      throw new Error(`unsupported command: ${input.command}`);
  }
}

async function main() {
  try {
    const input = await readInput();
    const result = await executeCommand(input);
    emit({ ok: true, result });
  } catch (error) {
    emitError(error);
    if (error instanceof Error && error.stack) {
      stderr.write(`${error.stack}\n`);
    }
    exit(1);
  }
}

await main();
