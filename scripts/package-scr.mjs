// 把 release 构建产物复制一份改名 .scr。
// 屏保与应用必须是字节级相同的二进制，只靠文件名区分。
import { copyFile, mkdir, readFile, readdir, stat } from "node:fs/promises";
import { join, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(new URL(".", import.meta.url)));

// cargo 可能被 CARGO_TARGET_DIR 重定向，先看环境变量再看默认位置。
const targetRoot = process.env.CARGO_TARGET_DIR
  ? resolve(process.env.CARGO_TARGET_DIR)
  : join(root, "src-tauri", "target");
const releaseDir = join(targetRoot, "release");
const outDir = join(root, "dist-scr");

const CANDIDATES = ["flipclock.exe", "FlipClock.exe"];

async function findBinary() {
  for (const name of CANDIDATES) {
    const path = join(releaseDir, name);
    try {
      const info = await stat(path);
      if (info.isFile()) return path;
    } catch {
      // 继续找下一个
    }
  }
  return null;
}

const binary = await findBinary();
if (!binary) {
  console.error(
    `找不到 release 构建产物。先运行 pnpm build:app，再运行本脚本。\n已查找：${CANDIDATES.join(", ")} → ${releaseDir}`,
  );
  process.exit(1);
}

// 没启用 custom-protocol 的 release 二进制不会内嵌前端，而是去连 devUrl：
// 双击只会看到一个「localhost 拒绝连接」的错误页。
// 判断办法：内嵌资源的文件名（带哈希）会作为资源路径出现在二进制里，开发构建则没有。
const bytes = await readFile(binary);
const assetsDir = join(root, "dist", "assets");
let assetNames = [];
try {
  assetNames = (await readdir(assetsDir)).filter((name) => /-[A-Za-z0-9_-]{6,}\./.test(name));
} catch {
  console.error(`找不到前端产物 ${assetsDir}，先运行 pnpm build。`);
  process.exit(1);
}

if (assetNames.length > 0 && !assetNames.some((name) => bytes.includes(Buffer.from(name)))) {
  console.error(
    `这个产物是开发模式构建，没有内嵌前端资源，装成屏保只会显示「localhost 拒绝连接」。\n` +
      `请这样构建：pnpm build:app（等价于 tauri build --no-bundle）\n` +
      `或手动：cargo build --release --features custom-protocol`,
  );
  process.exit(1);
}

await mkdir(outDir, { recursive: true });
await copyFile(binary, join(outDir, "FlipClock.exe"));
await copyFile(binary, join(outDir, "FlipClock.scr"));

const { size } = await stat(join(outDir, "FlipClock.scr"));
console.log(`已产出：
  ${join(outDir, "FlipClock.exe")}
  ${join(outDir, "FlipClock.scr")}  (${(size / 1024 / 1024).toFixed(1)} MB)

用法：在本机运行 FlipClock.exe /c，点「安装并设为当前屏保」，
它会把自身复制到 %LOCALAPPDATA%\\FlipClock\\FlipClock.scr 并指向当前用户。`);
