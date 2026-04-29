import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import jsonWorker from "monaco-editor/esm/vs/language/json/json.worker?worker";
import cssWorker from "monaco-editor/esm/vs/language/css/css.worker?worker";
import htmlWorker from "monaco-editor/esm/vs/language/html/html.worker?worker";
import tsWorker from "monaco-editor/esm/vs/language/typescript/ts.worker?worker";

import * as monaco from "monaco-editor/esm/vs/editor/editor.api";
import { getThemePreference, isDarkThemeMode, THEME_CHANGE_EVENT } from "@/lib/theme";
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

const MONACO_LIGHT_THEME = "codex-ai-light";
const MONACO_DARK_THEME = "codex-ai-dark";

let monacoConfigured = false;
let monacoThemesDefined = false;
let monacoThemeListenerBound = false;

type ThemeChangeDetail = {
  isDark?: boolean;
};

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

function defineMonacoThemes() {
  if (monacoThemesDefined) {
    return;
  }

  monaco.editor.defineTheme(MONACO_LIGHT_THEME, {
    base: "vs",
    inherit: true,
    rules: [],
    colors: {
      "editor.background": "#ffffff",
      "editor.foreground": "#18181b",
      "editorGutter.background": "#ffffff",
      "editorLineNumber.foreground": "#71717a",
      "editorLineNumber.activeForeground": "#3f3f46",
      "editor.lineHighlightBackground": "#f4f4f5",
      "editor.selectionBackground": "#bfdbfe",
      "editor.inactiveSelectionBackground": "#dbeafe",
      "editorCursor.foreground": "#18181b",
      "editorWidget.background": "#ffffff",
      "editorWidget.border": "#e4e4e7",
      "input.background": "#ffffff",
      "input.border": "#d4d4d8",
      "dropdown.background": "#ffffff",
      "dropdown.border": "#e4e4e7",
      "scrollbarSlider.background": "#71717a40",
      "scrollbarSlider.hoverBackground": "#71717a66",
      "scrollbarSlider.activeBackground": "#71717a80",
      "diffEditor.insertedLineBackground": "#22c55e1a",
      "diffEditor.removedLineBackground": "#ef44441a",
      "diffEditor.insertedTextBackground": "#22c55e33",
      "diffEditor.removedTextBackground": "#ef444433",
      "diffEditor.border": "#e4e4e7",
    },
  });

  monaco.editor.defineTheme(MONACO_DARK_THEME, {
    base: "vs-dark",
    inherit: true,
    rules: [],
    colors: {
      "editor.background": "#18181b",
      "editor.foreground": "#f4f4f5",
      "editorGutter.background": "#18181b",
      "editorLineNumber.foreground": "#71717a",
      "editorLineNumber.activeForeground": "#d4d4d8",
      "editor.lineHighlightBackground": "#27272a",
      "editor.selectionBackground": "#2563eb66",
      "editor.inactiveSelectionBackground": "#33415566",
      "editorCursor.foreground": "#f4f4f5",
      "editorWidget.background": "#18181b",
      "editorWidget.border": "#3f3f46",
      "input.background": "#27272a",
      "input.border": "#3f3f46",
      "dropdown.background": "#27272a",
      "dropdown.border": "#3f3f46",
      "scrollbarSlider.background": "#a1a1aa33",
      "scrollbarSlider.hoverBackground": "#a1a1aa4d",
      "scrollbarSlider.activeBackground": "#a1a1aa66",
      "diffEditor.insertedLineBackground": "#22c55e24",
      "diffEditor.removedLineBackground": "#ef444424",
      "diffEditor.insertedTextBackground": "#22c55e38",
      "diffEditor.removedTextBackground": "#ef444438",
      "diffEditor.border": "#3f3f46",
    },
  });

  monacoThemesDefined = true;
}

export function getMonacoThemeName(isDark = isDarkThemeMode(getThemePreference())) {
  return isDark ? MONACO_DARK_THEME : MONACO_LIGHT_THEME;
}

function getEventThemeIsDark(event: Event) {
  const detail = (event as CustomEvent<ThemeChangeDetail>).detail;
  if (typeof detail?.isDark === "boolean") {
    return detail.isDark;
  }

  return isDarkThemeMode(getThemePreference());
}

function applyMonacoTheme(isDark = isDarkThemeMode(getThemePreference())) {
  monaco.editor.setTheme(getMonacoThemeName(isDark));
}

function ensureMonacoThemeBridge() {
  defineMonacoThemes();
  applyMonacoTheme();

  if (monacoThemeListenerBound || typeof window === "undefined") {
    return;
  }

  window.addEventListener(THEME_CHANGE_EVENT, (event) => {
    applyMonacoTheme(getEventThemeIsDark(event));
  });
  monacoThemeListenerBound = true;
}

export async function loadMonaco() {
  ensureMonacoEnvironment();
  ensureMonacoThemeBridge();
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
