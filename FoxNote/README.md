# FoxNote App Workspace

FoxNote is a Tauri desktop note app with a Rust backend and Vue 3 + Vuetify frontend.
This workspace currently implements Wave 1 + Wave 2 foundations:

- Block-based TOML note storage (`note.toml` in each note folder)
- Folder + note CRUD wired through Tauri commands
- Typst block editing with a lightweight preview panel
- Nested tag indexing in SQLite (`tag-index.sqlite3`)
- Tag bridge export/import TOML (`tags-index.toml`) for Git-friendly sync handoff
- Tag explorer and note filtering by tag branch
- Baseline frontend and Rust test setup

## Development

Install dependencies:

```bash
pnpm install
```

Run the frontend in browser mode:

```bash
pnpm dev
```

Run Tauri desktop app:

```bash
pnpm tauri dev
```

## Quality checks

```bash
pnpm lint
pnpm test
pnpm test:rust
```

All checks:

```bash
pnpm check
```
