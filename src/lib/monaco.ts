import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import jsonWorker from "monaco-editor/esm/vs/language/json/json.worker?worker";
import cssWorker from "monaco-editor/esm/vs/language/css/css.worker?worker";
import htmlWorker from "monaco-editor/esm/vs/language/html/html.worker?worker";
import tsWorker from "monaco-editor/esm/vs/language/typescript/ts.worker?worker";

import * as monaco from "monaco-editor/esm/vs/editor/editor.api";
import "monaco-editor/esm/vs/language/json/monaco.contribution";
import "monaco-editor/esm/vs/language/css/monaco.contribution";
import "monaco-editor/esm/vs/language/html/monaco.contribution";
import "monaco-editor/esm/vs/language/typescript/monaco.contribution";
import "monaco-editor/esm/vs/basic-languages/markdown/markdown.contribution";
import "monaco-editor/esm/vs/basic-languages/python/python.contribution";
import "monaco-editor/esm/vs/basic-languages/go/go.contribution";
import "monaco-editor/esm/vs/basic-languages/java/java.contribution";
import "monaco-editor/esm/vs/basic-languages/rust/rust.contribution";
import "monaco-editor/esm/vs/basic-languages/shell/shell.contribution";
import "monaco-editor/esm/vs/basic-languages/sql/sql.contribution";
import "monaco-editor/esm/vs/basic-languages/xml/xml.contribution";
import "monaco-editor/esm/vs/basic-languages/yaml/yaml.contribution";
import "monaco-editor/esm/vs/basic-languages/ini/ini.contribution";

let monacoConfigured = false;

function ensureMonacoEnvironment() {
  if (monacoConfigured) {
    return;
  }

  (
    self as typeof globalThis & {
      MonacoEnvironment?: {
        getWorker: (_workerId: string, label: string) => Worker;
      };
    }
  ).MonacoEnvironment = {
    getWorker(_workerId: string, label: string) {
      switch (label) {
        case "json":
          return new jsonWorker();
        case "css":
        case "scss":
        case "less":
          return new cssWorker();
        case "html":
        case "handlebars":
        case "razor":
          return new htmlWorker();
        case "typescript":
        case "javascript":
          return new tsWorker();
        default:
          return new editorWorker();
      }
    },
  };

  monacoConfigured = true;
}

export async function loadMonaco() {
  ensureMonacoEnvironment();
  return monaco;
}

export function detectMonacoLanguage(filePath: string): string {
  const normalized = filePath.toLowerCase();
  if (normalized.endsWith(".ts")) return "typescript";
  if (normalized.endsWith(".tsx")) return "typescript";
  if (normalized.endsWith(".js")) return "javascript";
  if (normalized.endsWith(".jsx")) return "javascript";
  if (normalized.endsWith(".json")) return "json";
  if (normalized.endsWith(".css")) return "css";
  if (normalized.endsWith(".scss")) return "scss";
  if (normalized.endsWith(".less")) return "less";
  if (normalized.endsWith(".html")) return "html";
  if (normalized.endsWith(".md")) return "markdown";
  if (normalized.endsWith(".rs")) return "rust";
  if (normalized.endsWith(".go")) return "go";
  if (normalized.endsWith(".py")) return "python";
  if (normalized.endsWith(".java")) return "java";
  if (normalized.endsWith(".yaml") || normalized.endsWith(".yml")) return "yaml";
  if (normalized.endsWith(".xml")) return "xml";
  if (normalized.endsWith(".sh") || normalized.endsWith(".bash") || normalized.endsWith(".zsh")) return "shell";
  if (normalized.endsWith(".sql")) return "sql";
  if (normalized.endsWith(".toml")) return "ini";
  return "plaintext";
}
