# try-rs - Rust Port of try

**Fast, native implementation of [try](https://github.com/tobi/try) - fresh directories for every vibe** 🏠

*Your experiments deserve a home.*

---

## About

This is a complete Rust port of [Tobi Lütke's `try`](https://github.com/tobi/try) - an ephemeral workspace manager that helps you organize your coding experiments with fuzzy search, smart scoring, and date-prefixed directories.

**Why a Rust port?**
- ⚡ **Native binary** - No Ruby runtime required, ships as a single executable
- 🚀 **Fast startup** - Instant launch, no interpreter overhead
- 📦 **Easy distribution** - Install via Homebrew, or download a single binary
- 🎯 **100% feature parity** - Matches Ruby version behavior exactly (passes all 387/387 assertions in original test suite)

## Original Credit

This project is a port of the original Ruby implementation by **Tobi Lütke** ([@tobi](https://github.com/tobi)).

- **Original project**: https://github.com/tobi/try
- **Author**: Tobi Lütke
- **License**: MIT (preserved in this port)

All credit for the design, UX, and concept goes to the original author. This Rust implementation aims to provide a faster, dependency-free alternative while maintaining complete compatibility.

---

## Features

Instantly navigate through all your experiment directories with:
- **Fuzzy search** that just works
- **Smart sorting** - recently used stuff bubbles to the top
- **Auto-dating** - creates directories like `2025-03-12-redis-experiment`
- **Git integration** - Clone repos or create worktrees with single commands
- **Zero config** - works out of the box with sensible defaults

## Installation

### Homebrew (Recommended)

\`\`\`bash
brew tap BrainBuzzer/try-rs
brew install try-rs
\`\`\`

Then add to your shell:

\`\`\`bash
# Bash/Zsh - add to .zshrc or .bashrc
eval "$(try init)"

# Fish - add to config.fish
try init | source
\`\`\`

### From Source

\`\`\`bash
git clone https://github.com/BrainBuzzer/try-rs
cd try-rs
cargo build --release
cp target/release/try ~/.local/bin/
\`\`\`

Then add the shell integration:

\`\`\`bash
# Bash/Zsh
eval "$(try init ~/src/tries)"

# Fish
try init ~/src/tries | source
\`\`\`

---

## Usage

\`\`\`bash
try                                          # Browse all experiments
try redis                                    # Jump to redis experiment or create new
try new api                                  # Start with "2025-03-12-new-api"
try . [name]                                 # Create dated worktree dir for current repo
try clone https://github.com/user/repo.git  # Clone repo into date-prefixed directory
try https://github.com/user/repo.git        # Shorthand for clone (same as above)
try --help                                   # See all options
\`\`\`

### Keyboard Shortcuts

- \`↑/↓\` or \`Ctrl-P/N/J/K\` - Navigate
- \`Enter\` - Select or create
- \`Backspace\` - Delete character
- \`Ctrl-D\` - Delete directory (with confirmation)
- \`Ctrl-R\` - Rename directory
- \`Ctrl-G\` - Graduate (promote to projects folder)
- \`ESC\` - Cancel

### Git Integration

**Clone repositories:**
\`\`\`bash
# Clone with auto-generated directory name
try clone https://github.com/tobi/try.git
# Creates: 2025-03-12-tobi-try

# Clone with custom name
try clone https://github.com/tobi/try.git my-fork
# Creates: my-fork
\`\`\`

**Create worktrees:**
\`\`\`bash
# Inside a git repo
try .                    # Creates dated worktree from current repo
try . feature-name       # Custom name: 2025-03-12-feature-name
\`\`\`

Supported git URI formats:
- \`https://github.com/user/repo.git\` (HTTPS GitHub)
- \`https://gitlab.com/user/repo.git\` (HTTPS GitLab)
- \`git@github.com:user/repo.git\` (SSH GitHub)
- \`git@gitlab.com:user/repo.git\` (SSH GitLab)
- \`git@host.com:user/repo.git\` (SSH other hosts)

---

## Configuration

Set \`TRY_PATH\` to change where experiments are stored:

\`\`\`bash
export TRY_PATH=~/code/experiments
\`\`\`

Default: \`~/src/tries\`

Set \`TRY_PROJECTS\` for graduation destination:

\`\`\`bash
export TRY_PROJECTS=~/projects
\`\`\`

Default: parent directory of \`TRY_PATH\`

---

## Comparison with Ruby Version

| Feature | Ruby | Rust (this port) |
|---------|------|------------------|
| Runtime | Requires Ruby | Native binary, no dependencies |
| Startup time | ~100-150ms | ~5-10ms |
| Installation | gem install + Ruby | Homebrew or single binary |
| Feature parity | 100% | 100% (387/387 tests passing) ✅ |
| Test suite | 37 tests, 387 assertions | Passes 387/387 assertions ✅ |

**All features working:**
- ✅ Interactive fuzzy selector with scoring
- ✅ Create, rename, delete directories
- ✅ Git clone with date-prefixing
- ✅ Git worktree creation
- ✅ Graduate/symlink feature (move tries to projects)
- ✅ Shell integration (bash/zsh/fish)
- ✅ All keyboard shortcuts
- ✅ NO_COLOR support
- ✅ Environment variable configuration

---

## Development

### Building

\`\`\`bash
cargo build --release
\`\`\`

Binary will be at \`target/release/try\`

### Testing

This port uses the **original Ruby test suite** to ensure 1:1 compatibility:

\`\`\`bash
# Run full test suite
./try/spec/tests/runner.sh ./target/release/try

# Status: ✅ 387/387 assertions passing (100% coverage)
\`\`\`

### Architecture

- **No clap** - Manual arg parsing to match Ruby's exact CLI semantics
- **No ratatui/tui-rs** - Direct crossterm + stderr-only rendering for compatibility
- **No tokio/async** - Blocking IO with crossterm event polling (100ms)
- **Binary name**: \`try\` (configured via \`[[bin]]\` in Cargo.toml)
- **Edition**: Rust 2021

---

## License

MIT License - Same as the original Ruby version

Copyright (c) 2025 Tobi Lütke (original Ruby implementation)  
Copyright (c) 2025 Aditya Chinchure (Rust port)

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

---

## Contributing

This port aims to maintain 100% compatibility with the original Ruby version. 

**Before submitting PRs:**
1. Ensure changes match Ruby behavior (check \`try/try.rb\` reference)
2. Run the test suite: \`./try/spec/tests/runner.sh ./target/release/try\`
3. Add unit tests for new functionality

---

## Links

- **This Rust port**: https://github.com/BrainBuzzer/try-rs
- **Original Ruby version**: https://github.com/tobi/try
- **Original author**: [@tobi](https://github.com/tobi) (Tobi Lütke)

---

*Built for developers with ADHD by developers with ADHD.*  
*Your experiments deserve a home.* 🏠
