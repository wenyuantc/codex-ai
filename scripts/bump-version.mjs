import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const nextVersion = process.argv[2];

if (!nextVersion) {
  console.error("用法: npm run bump-version -- <version>");
  process.exit(1);
}

if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(nextVersion)) {
  console.error(`无效版本号: ${nextVersion}`);
  process.exit(1);
}

const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const updatedFiles = [];

function replaceInFile(relativePath, replacer) {
  const filePath = path.join(rootDir, relativePath);
  const original = fs.readFileSync(filePath, "utf8");
  const updated = replacer(original);

  if (updated === original) {
    return;
  }

  fs.writeFileSync(filePath, updated);
  updatedFiles.push(relativePath);
}

replaceInFile("src-tauri/Cargo.toml", (content) =>
  replaceAfterMarker(content, "[package]", /^version\s*=\s*"[^"]+"$/m, `version = "${nextVersion}"`),
);

replaceInFile("src-tauri/Cargo.lock", (content) =>
  replaceAfterMarker(
    content,
    '[[package]]\nname = "codex-ai"',
    /^version = "[^"]+"$/m,
    `version = "${nextVersion}"`,
  ),
);

replaceInFile("package.json", (content) =>
  replaceLine(content, /^  "version": "[^"]+",$/m, `  "version": "${nextVersion}",`),
);

replaceInFile("package-lock.json", (content) =>
  replaceAfterMarker(
    replaceLine(content, /^  "version": "[^"]+",$/m, `  "version": "${nextVersion}",`),
    '  "": {',
    /^      "version": "[^"]+",$/m,
    `      "version": "${nextVersion}",`,
  ),
);

replaceInFile("src-tauri/tauri.conf.json", (content) =>
  replaceLine(content, /^  "version": "[^"]+",$/m, `  "version": "${nextVersion}",`),
);

if (updatedFiles.length === 0) {
  console.log(`版本号已经是 ${nextVersion}，无需更新`);
  process.exit(0);
}

console.log(`版本号已更新为 ${nextVersion}`);
for (const file of updatedFiles) {
  console.log(`- ${file}`);
}

function replaceAfterMarker(content, marker, pattern, replacement) {
  const markerIndex = content.indexOf(marker);

  if (markerIndex === -1) {
    return content;
  }

  const beforeMarker = content.slice(0, markerIndex);
  const afterMarker = content.slice(markerIndex);
  const match = afterMarker.match(pattern);

  if (!match || match[0] === replacement) {
    return content;
  }

  const updatedAfterMarker = afterMarker.replace(pattern, replacement);

  return beforeMarker + updatedAfterMarker;
}

function replaceLine(content, pattern, replacement) {
  const match = content.match(pattern);

  if (!match || match[0] === replacement) {
    return content;
  }

  return content.replace(pattern, replacement);
}
