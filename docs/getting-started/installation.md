# Installation & Setup

## Prerequisites

- **Node.js** 20 or later
- **npm** 9 or later

Verify your installation:

```bash
node --version   # Should print v20.x.x or higher
npm --version    # Should print 9.x.x or higher
```

## Clone & Install

```bash
git clone https://github.com/biswajith/nexus.git
cd nexus
npm install
```

This installs dependencies for all packages in the monorepo (root, `@nexus/core`, and `@nexus/desktop`).

## Build & Run

```bash
# Build the core engine
npm run build:core

# Start the desktop app in development mode
npm run dev
```

Or use the convenience script:

```bash
./start.sh
```

The Vite dev server starts at `http://localhost:5173`. The Electron main process loads this URL in development mode.

## Production Build

```bash
# Build everything (core + desktop)
npm run build
```

The production build outputs to `packages/core/dist/` and `packages/desktop/dist/`.

## Available Scripts

| Command | Description |
|---|---|
| `npm run dev` | Start Vite dev server for the renderer |
| `npm run build` | Build core + desktop for production |
| `npm run build:core` | Build the core package only |
| `npm run build:desktop` | Build the desktop package only |
| `npm run typecheck` | Type-check all packages |
| `npm run lint` | Lint all packages |
| `npm run clean` | Remove dist folders and Vite cache |

## Project Structure

```
nexus/
├── packages/
│   ├── core/         # Pure TypeScript engine (no UI dependency)
│   └── desktop/      # Electron + React application
├── docs/             # Product documentation
├── package.json      # Monorepo root (npm workspaces)
├── turbo.json        # Turborepo build orchestration
└── tsconfig.base.json
```

## System Requirements

| Requirement | Minimum |
|---|---|
| Node.js | 20.0.0 |
| npm | 9.0.0 |
| OS | macOS 12+, Windows 10+, Ubuntu 20.04+ |
| RAM | 4 GB (8 GB recommended) |
| Disk | 500 MB for installation |
