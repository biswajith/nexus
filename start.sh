#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${CYAN}╔═══════════════════════════════════════╗${NC}"
echo -e "${CYAN}║           ${GREEN}Nexus API Client${CYAN}            ║${NC}"
echo -e "${CYAN}║     Local-first. Offline. Yours.      ║${NC}"
echo -e "${CYAN}╚═══════════════════════════════════════╝${NC}"
echo ""

# Check Node.js
if ! command -v node &>/dev/null; then
    echo -e "${RED}Error: Node.js is not installed. Please install Node.js 20+.${NC}"
    exit 1
fi

NODE_VERSION=$(node -v | sed 's/v//' | cut -d. -f1)
if [ "$NODE_VERSION" -lt 20 ]; then
    echo -e "${RED}Error: Node.js 20+ required (found v$(node -v)).${NC}"
    exit 1
fi
echo -e "${GREEN}✓${NC} Node.js $(node -v)"

# Check npm
if ! command -v npm &>/dev/null; then
    echo -e "${RED}Error: npm is not installed.${NC}"
    exit 1
fi
echo -e "${GREEN}✓${NC} npm $(npm -v)"

# Install dependencies
echo ""
echo -e "${YELLOW}Installing dependencies...${NC}"
npm install --silent 2>/dev/null || npm install
echo -e "${GREEN}✓${NC} Dependencies installed"

# Build core package
echo ""
echo -e "${YELLOW}Building core package...${NC}"
npm run build:core --silent 2>/dev/null || npm run build:core
echo -e "${GREEN}✓${NC} Core package built"

# Create data directory
mkdir -p "$HOME/.nexus/collections" "$HOME/.nexus/history"
echo -e "${GREEN}✓${NC} Data directory ready (~/.nexus/)"

# Compile Electron main process + bundle preload as CJS
echo ""
echo -e "${YELLOW}Compiling Electron main process...${NC}"
npm run build:electron --workspace=packages/desktop 2>/dev/null || npm run build:electron --workspace=packages/desktop
echo -e "${GREEN}✓${NC} Electron compiled"

# Start
echo ""
echo -e "${CYAN}Starting Nexus...${NC}"
echo -e "  Renderer: ${GREEN}http://localhost:5173${NC}"
echo -e "  Press ${YELLOW}Cmd+K${NC} for command palette"
echo ""

# Start Vite dev server in background, then Electron
npm run dev --workspace=packages/desktop &
VITE_PID=$!

sleep 3

(cd packages/desktop && npx electron . --dev 2>/dev/null) &
ELECTRON_PID=$!

cleanup() {
    echo ""
    echo -e "${YELLOW}Shutting down Nexus...${NC}"
    kill $VITE_PID 2>/dev/null || true
    kill $ELECTRON_PID 2>/dev/null || true
    wait $VITE_PID 2>/dev/null || true
    wait $ELECTRON_PID 2>/dev/null || true
    echo -e "${GREEN}Goodbye.${NC}"
}

trap cleanup EXIT INT TERM

wait $ELECTRON_PID 2>/dev/null || true
