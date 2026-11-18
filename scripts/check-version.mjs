// 版本号一致性检查。
//
// 版本号在三个地方各写一份：package.json、Cargo.toml、tauri.conf.json。
// 其中只有 tauri.conf.json 会进到产物里（updater 用它判断「我是哪一版」），
// 另外两处漂移不会报错，只会让「关于」页和安装包对不上——所以要在这里拦住。
//
// 用法：
//   node scripts/check-version.mjs                # 只查三处一致
//   node scripts/check-version.mjs --tag v1.0.0   # 再查 tag 与版本一致（CI 用）

import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

function argValue(flag) {
  const index = process.argv.indexOf(flag);
  return index >= 0 ? process.argv[index + 1] : undefined;
}

function readText(relativePath) {
  return readFileSync(join(root, relativePath), "utf8");
}

function jsonVersion(relativePath) {
  return JSON.parse(readText(relativePath)).version;
}

/** Cargo 的 version 只在 [package] 段落里那一行，别撞上依赖的版本号。 */
function cargoVersion(relativePath) {
  const text = readText(relativePath);
  const start = text.indexOf("[package]");
  if (start < 0) throw new Error(`${relativePath} 里找不到 [package]`);
  const matched = text.slice(start).match(/^version\s*=\s*"([^"]+)"/m);
  if (!matched) throw new Error(`${relativePath} 的 [package] 里没有 version`);
  return matched[1];
}

const versions = {
  "package.json": jsonVersion("package.json"),
  "src-tauri/Cargo.toml": cargoVersion("src-tauri/Cargo.toml"),
  "src-tauri/tauri.conf.json": jsonVersion("src-tauri/tauri.conf.json"),
};

const distinct = new Set(Object.values(versions));
if (distinct.size !== 1) {
  console.error("版本号不一致：");
  for (const [file, version] of Object.entries(versions)) {
    console.error(`  ${file.padEnd(24)} ${version}`);
  }
  process.exit(1);
}

const version = versions["src-tauri/tauri.conf.json"];
const tag = argValue("--tag") ?? process.env.GITHUB_REF_NAME ?? "";

if (tag) {
  const expected = tag.replace(/^v/, "");
  if (expected !== version) {
    console.error(`tag 与版本号不一致：tag=${tag}，tauri.conf.json=${version}`);
    console.error("先跑 pnpm release 让 tag 由版本号产生，别手打 tag。");
    process.exit(1);
  }
}

console.log(`版本一致：${version}${tag ? `（tag ${tag}）` : ""}`);
