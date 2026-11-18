// 发版：升版本号 → 提交 → 打 tag → 推送。
//
// 只改「版本」这一件事，发布说明由人来写：
// 先在 CHANGELOG.md 里加 `## [x.y.z] - 日期` 段落，再跑本脚本。
// 脚本找不到对应段落就直接停——Release Notes 不该由脚本编。
//
// 用法：
//   pnpm release                 # 交互式，默认建议补丁位 +1
//   pnpm release -- --version 1.1.0
//   pnpm release -- --yes        # 不确认，直接执行（CI/自动化才用）

import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { createInterface } from "node:readline/promises";
import process from "node:process";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const RELEASE_FILES = [
  "package.json",
  "src-tauri/Cargo.toml",
  "src-tauri/tauri.conf.json",
  "CHANGELOG.md",
];
const SEMVER = /^\d+\.\d+\.\d+$/;

function argValue(flag) {
  const index = process.argv.indexOf(flag);
  return index >= 0 ? process.argv[index + 1] : undefined;
}

const assumeYes = process.argv.includes("--yes");

function git(args, options = {}) {
  const out = execFileSync("git", args, { cwd: root, encoding: "utf8", ...options });
  // stdio 继承时 execFileSync 返回 null，别对它调 trim。
  return typeof out === "string" ? out.trim() : "";
}

function readJson(relativePath) {
  return JSON.parse(readFileSync(join(root, relativePath), "utf8"));
}

function currentVersion() {
  return readJson("src-tauri/tauri.conf.json").version;
}

function bumpPatch(version) {
  const [major, minor, patch] = version.split(".").map(Number);
  return `${major}.${minor}.${patch + 1}`;
}

/**
 * 只替换值，不重新序列化整个文件：JSON.stringify 会把数组和内联对象全部摊开，
 * 每发一版都留下几百行无意义的 diff。
 */
function writeJsonVersion(relativePath, version) {
  const path = join(root, relativePath);
  const text = readFileSync(path, "utf8");
  const next = text.replace(/("version"\s*:\s*")[^"]+(")/, `$1${version}$2`);
  if (next === text) throw new Error(`${relativePath} 里没有可替换的 version`);
  writeFileSync(path, next);
}

function writeCargoVersion(version) {
  const path = join(root, "src-tauri/Cargo.toml");
  const text = readFileSync(path, "utf8");
  const start = text.indexOf("[package]");
  if (start < 0) throw new Error("Cargo.toml 里找不到 [package]");
  const next =
    text.slice(0, start) +
    text
      .slice(start)
      .replace(/^version\s*=\s*"[^"]+"/m, `version = "${version}"`);
  writeFileSync(path, next);
}

function changelogHasSection(version) {
  const text = readFileSync(join(root, "CHANGELOG.md"), "utf8");
  // 接受 `## [1.1.0] - 2026-11-20`、`## v1.1.0` 两种写法。
  const heading = new RegExp(
    `^##\\s+\\[?v?${version.replace(/\./g, "\\.")}\\]?(\\s|$)`,
    "m",
  );
  return heading.test(text);
}

function ensureTagFree(tag) {
  try {
    git(["rev-parse", "--verify", `refs/tags/${tag}`], { stdio: "pipe" });
  } catch {
    return;
  }
  throw new Error(`tag ${tag} 已存在`);
}

function dirtyPaths() {
  // -z 关掉路径的引号转义，中文文件名才不会被包成 "..."。
  return git(["status", "--porcelain", "-z"])
    .split("\0")
    .filter(Boolean)
    .map((entry) => entry.slice(3).trim())
    .filter(Boolean);
}

async function ask(question, fallback) {
  if (assumeYes) return fallback;
  const rl = createInterface({ input: process.stdin, output: process.stdout });
  try {
    const answer = (await rl.question(`${question} [${fallback}]: `)).trim();
    return answer || fallback;
  } finally {
    rl.close();
  }
}

async function main() {
  const from = currentVersion();
  const to = (argValue("--version") ?? (await ask("新版本号", bumpPatch(from)))).trim();

  if (!SEMVER.test(to)) throw new Error(`不是合法的语义化版本：${to}`);
  if (to === from) throw new Error(`新版本号与当前版本相同：${to}`);
  ensureTagFree(`v${to}`);

  if (!changelogHasSection(to)) {
    throw new Error(
      `CHANGELOG.md 里没有 ${to} 的段落。\n` +
        `先加一行 \`## [${to}] - YYYY-MM-DD\` 并写清这一版改了什么，再重新运行。`,
    );
  }

  // 只允许 CHANGELOG.md 是脏的：它本来就是这一步的输入。
  const dirty = dirtyPaths().filter((path) => path !== "CHANGELOG.md");
  if (dirty.length) {
    throw new Error(`工作区还有未提交的改动，先处理掉：\n  ${dirty.join("\n  ")}`);
  }

  console.log("");
  console.log(`  当前版本  ${from}`);
  console.log(`  目标版本  ${to}`);
  console.log(`  将提交    ${RELEASE_FILES.join(", ")}`);
  console.log(`  将推送    ${git(["remote", "get-url", "origin"])}`);
  console.log("");

  if (!assumeYes) {
    const answer = (await ask("确认发版？(y/N)", "n")).toLowerCase();
    if (answer !== "y" && answer !== "yes") {
      console.log("已取消，什么都没改。");
      return;
    }
  }

  writeJsonVersion("package.json", to);
  writeCargoVersion(to);
  writeJsonVersion("src-tauri/tauri.conf.json", to);

  // 版本号刚写完，先自检一遍再提交；不一致就不该产生 commit。
  execFileSync(process.execPath, ["scripts/check-version.mjs", "--tag", `v${to}`], {
    cwd: root,
    stdio: "inherit",
  });

  git(["add", ...RELEASE_FILES]);
  git(["commit", "-m", `chore(release): v${to}`]);
  git(["tag", "-a", `v${to}`, "-m", `FlipClock v${to}`]);

  const branch = git(["rev-parse", "--abbrev-ref", "HEAD"]);
  git(["push", "origin", branch], { stdio: "inherit" });
  git(["push", "origin", `v${to}`], { stdio: "inherit" });

  console.log("");
  console.log(`已推送 v${to}。tag 会触发发布流水线，发布说明取 CHANGELOG 里这一版的段落。`);
}

main().catch((error) => {
  console.error(`\n发版中止：${error.message}`);
  process.exit(1);
});
