# Repository Guidelines

## Project Structure & Module Organization
The app is split between a Vite/React frontend and a Tauri/Rust desktop shell. Frontend source lives in `src/`: page-level routes are in `src/pages`, reusable UI in `src/components`, shared helpers in `src/lib`, and Zustand state in `src/stores`. Static assets belong in `public/` or `src/assets/`. Native desktop code lives in `src-tauri/src`, with database code under `src-tauri/src/db` and Codex process management under `src-tauri/src/codex`. Treat `.omx/` and `src-tauri/target/` as generated/runtime state, not hand-edited source.

## Build, Test, and Development Commands
- `npm run dev`: start the Vite frontend for browser-only development.
- `npm run build`: run TypeScript compilation and produce the frontend bundle.
- `npm run preview`: serve the production frontend bundle locally.
- `npm run tauri dev`: launch the desktop app with the Rust backend and hot-reloaded frontend.
- `npm run tauri build`: build a distributable desktop bundle.
- `cargo test --manifest-path src-tauri/Cargo.toml`: run Rust-side tests and compile checks for the Tauri layer.

## Coding Style & Naming Conventions
Use TypeScript with 2-space indentation and Rust with default `rustfmt` formatting. Keep React components, pages, and dialogs in PascalCase files such as `CreateTaskDialog.tsx`; stores, utilities, and module helpers use camelCase filenames such as `taskStore.ts` and `database.ts`. Prefer named exports for shared React components and colocate domain UI under folders like `src/components/tasks`. Reuse existing utility primitives in `src/components/ui` and `src/lib/utils.ts` before adding new abstractions.

## Testing Guidelines
This repository does not yet define a frontend test runner, so every change should at minimum pass `npm run build` and receive a manual smoke test in `npm run tauri dev`. When adding automated tests, place frontend tests beside the feature or under `src/__tests__`, and keep Rust tests in the relevant `src-tauri/src/*` module with `#[cfg(test)]`. Prioritize coverage for stores, database access, and Codex process management.

## Commit & Pull Request Guidelines
Recent history uses concise Conventional Commit prefixes such as `feat:` and `fix(codex):`; keep that pattern and write the subject around the reason for change. Pull requests should summarize user-visible behavior, list verification steps, and link the issue or task being addressed. Include screenshots or short recordings for UI changes, and call out database, capability, or process-management changes explicitly.
