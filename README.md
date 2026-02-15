# neocmakelsp-fast

Fast CMake Language Server based on Tower LSP and Tree-sitter.

This is a fork of [neocmakelsp](https://github.com/neocmakelsp/neocmakelsp) with performance optimizations.

## Features

- Intelligent code completion
- Real-time error detection and linting
- Go to definition (find_package, include, functions, macros)
- Hover documentation
- Code formatting (built-in and external via gersemi)
- Symbol provider and document outline
- Code actions
- Rename support
- Document links
- Watch file support (CMakeCache.txt)
- CLI tools for formatting and project analysis

## Installation

Download prebuilt binaries from [Releases](https://github.com/NikitolProject/neocmakelsp-fast/releases).

## Configuration

Configuration file: `.neocmake.toml` or `.neocmakelint.toml` in project root, or `$XDG_CONFIG_DIR/neocmakelsp/config.toml` for user-level config.

```toml
command_case = "lower_case" # or "upper_case"
enable_external_cmake_lint = true
line_max_words = 80

[format]
program = "gersemi"
args = ["--indent", "2"]
```

## Editor Support

### Neovim

```lua
local configs = require("lspconfig.configs")
local nvim_lsp = require("lspconfig")

configs.neocmake = {
    default_config = {
        cmd = { "neocmakelsp-fast", "--stdio" },
        filetypes = { "cmake" },
        root_dir = function(fname)
            return nvim_lsp.util.find_git_ancestor(fname)
        end,
        single_file_support = true,
        init_options = {
            format = { enable = true },
            lint = { enable = true },
            scan_cmake_in_package = true
        }
    }
}
nvim_lsp.neocmake.setup({})
```

Neovim 0.11+:

```lua
vim.lsp.config("neocmake", {})
vim.lsp.enable("neocmake")
```

### Helix

```toml
[[language]]
name = "cmake"
auto-format = true
language-servers = [{ name = "neocmakelsp-fast" }]

[language-server.neocmakelsp-fast]
command = "neocmakelsp-fast"
args = ["--stdio"]
```

### Emacs

```emacs-lisp
(use-package cmake-ts-mode
  :config
  (add-hook 'cmake-ts-mode-hook
    (defun setup-neocmakelsp ()
      (require 'eglot)
      (add-to-list 'eglot-server-programs `((cmake-ts-mode) . ("neocmakelsp-fast" "stdio")))
      (eglot-ensure))))
```

### Zed

Use the [zed-cmake](https://github.com/NikitolProject/zed-cmake) extension.

## LSP Init Options

```lua
init_options = {
    format = { enable = true },
    lint = { enable = true },
    scan_cmake_in_package = false,
    semantic_token = false
}
```

## CLI Usage

### Format

```bash
neocmakelsp-fast format [OPTIONS] <PATH>...
```

Options:
- `-o, --override` - override files in place

Reads `.editorconfig` for formatting settings:

```ini
[CMakeLists.txt]
indent_style = space
indent_size = 4
```

## Credits

Based on [neocmakelsp](https://github.com/neocmakelsp/neocmakelsp) by Decodertalkers.

## License

MIT
