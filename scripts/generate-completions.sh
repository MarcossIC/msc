#!/usr/bin/env bash
# Generate shell completions for MSC CLI

set -e

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}Generating shell completions for MSC...${NC}"

# Build the project first
echo "Building msc..."
cargo build --release

# Create completions directory if it doesn't exist
mkdir -p completions

# Binary path
MSC_BIN="./target/release/msc"

# Check if Windows (use .exe)
if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" ]]; then
    MSC_BIN="./target/release/msc.exe"
fi

# Generate completions for each shell
echo -e "\n${GREEN}Generating Bash completion...${NC}"
"$MSC_BIN" completions bash > completions/msc.bash

echo -e "${GREEN}Generating Zsh completion...${NC}"
"$MSC_BIN" completions zsh > completions/_msc

echo -e "${GREEN}Generating Fish completion...${NC}"
"$MSC_BIN" completions fish > completions/msc.fish

echo -e "${GREEN}Generating PowerShell completion...${NC}"
"$MSC_BIN" completions powershell > completions/_msc.ps1

echo -e "${GREEN}Generating Elvish completion...${NC}"
"$MSC_BIN" completions elvish > completions/msc.elv

echo -e "\n${BLUE}✓ Completions generated successfully in ./completions/${NC}"
echo -e "\nTo install:"
echo -e "  ${GREEN}Bash:${NC}       cp completions/msc.bash /usr/share/bash-completion/completions/msc"
echo -e "  ${GREEN}Zsh:${NC}        cp completions/_msc /usr/share/zsh/site-functions/_msc"
echo -e "  ${GREEN}Fish:${NC}       cp completions/msc.fish ~/.config/fish/completions/msc.fish"
echo -e "  ${GREEN}PowerShell:${NC} See completions/_msc.ps1 for installation instructions"
