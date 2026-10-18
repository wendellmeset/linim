# linim

Manage a project's open-source license in one command.

linim writes the root `LICENSE` file with the verbatim license text, records
the project's licensing decisions in a small committed `.linim` file, and
injects or updates SPDX-copyright headers at the top of every source file that
supports comments.

[![GitHub Downloads](https://img.shields.io/github/downloads/wendellmeset/linim/total?style=for-the-badge&logo=github&logoColor=white&label=TOTAL+DOWNLOADS&color=333333)](https://github.com/wendellmeset/linim/releases)
[![GitHub License](https://img.shields.io/github/license/wendellmeset/linim?style=for-the-badge&logo=github&logoColor=white&color=333333)](https://github.com/wendellmeset/linim/blob/main/LICENSE)
[![Latest Release](https://img.shields.io/github/v/release/wendellmeset/linim?style=for-the-badge&logo=github&logoColor=white&label=LATEST+RELEASE&color=333333)](https://github.com/wendellmeset/linim/releases/latest)

## Install

```sh
cargo build --release
# binary at target/release/linim
```

## Usage

```text
linim [OPTIONS] [PATH]
```

`PATH` defaults to the current directory.

| Option          | Meaning                                                            |
|-----------------|--------------------------------------------------------------------|
| `--new`         | Reconfigure from scratch, prompt for license and copyright, rewrite `LICENSE`, `.linim`, and every file's header. |
| `--no-headers`  | Write or update `LICENSE` only. Do not touch source file headers.  |
| `--verbose`     | Emit one line per updated file to stderr.                          |
| `-h, --help`    | Print help.                                                        |
| `-V, --version` | Print version.                                                     |

## How it works

On the first run linim:

1. If a recognized `LICENSE` already exists (an `SPDX-License-Identifier:`
   line, or a body matching a known license), it is used and the interactive
   license picker is skipped. Otherwise you pick a license.
2. You provide the copyright holder and year (prompted once).
3. linim writes `LICENSE` and `.linim`, then walks the tree and adds or updates
   the SPDX + copyright header at the top of every comment-capable source file.

On later runs, linim reuses `.linim` and makes no changes unless something
drifted: headers are compared against the expected form and only rewritten when
they differ, so repeated runs are effectively free.

Key properties:

- **Fast.** Skips `.git` and everything matched by `.gitignore`; skips files
  whose extension has no comment syntax; reads only the first 8 KB of each file
  on the fast path. Correct files are never rewritten.
- **Safe.** Files with two or more qualifying headers are reported and left
  alone, never modified. Shebangs, encoding cookies, doctypes, and other
  preambles are preserved above the injected header.
- **Deterministic.** Processing is parallel (rayon) but the report and
  per-file listing are in sorted path order.

## State file (`.linim`)

A small `key = value` file at the repo root, committed to git:

```text
# linim project state
spdx = MIT
holder = wendellmeset
year = 2026
headers = true
```

It stores only the project's licensing decisions. The truth is always derived
from the files themselves on each run, so broken or missing headers are caught
automatically and updates apply when the license, holder, or year changes.

## Development

- `cargo test` to run the unit + integration tests.
- `cargo build --release` to build the production binary.
- `tools/gen-comment-map/` is a dev tool that regenerates the extension to comment-style table from `data/tokei_languages.json`. Not part of the build.
