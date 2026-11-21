<div align="center">

# `mdr`

**Responsive Markdown compiler with theme tokens, macros, and local asset caching.**

<p>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="images/mdr/zero-dependencies-dark-86172033.svg" />
    <source media="(prefers-color-scheme: light)" srcset="images/mdr/zero-dependencies-light-f5d29842.svg" />
    <img alt="Zero Dependencies" src="images/mdr/zero-dependencies-dark-86172033.svg" />
  </picture>
  <a href="https://github.com/Powerisinschool/mdr/actions">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="images/mdr/ci-status-dark-b6bf56a9.svg" />
      <source media="(prefers-color-scheme: light)" srcset="images/mdr/ci-status-light-ecc1e703.svg" />
      <img alt="CI Status" src="images/mdr/ci-status-dark-b6bf56a9.svg" />
    </picture>
  </a>
  <a href="https://github.com/Powerisinschool/mdr/releases">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="images/mdr/releases-dark-255dc9d9.svg" />
      <source media="(prefers-color-scheme: light)" srcset="images/mdr/releases-light-1ce22ec5.svg" />
      <img alt="Releases" src="images/mdr/releases-dark-255dc9d9.svg" />
    </picture>
  </a>
  <a href="https://github.com/Powerisinschool/mdr">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="images/mdr/repository-dark-693bd5f9.svg" />
      <source media="(prefers-color-scheme: light)" srcset="images/mdr/repository-light-68c6d6af.svg" />
      <img alt="Repository" src="images/mdr/repository-dark-693bd5f9.svg" />
    </picture>
  </a>
</p>

</div>

---

### Why `mdr`?

GitHub Markdown supports dark and light theme switching through `<picture>` and `<source media="(prefers-color-scheme: ...)">` tags, but writing them by hand turns clean READMEs into repetitive HTML boilerplate[cite: 1].

- **Theme Tokens (`@theme`):** Declare paired dark and light values once and reference them anywhere with `{token}`[cite: 1].
- **Macros (`@def`):** Collapse repeated badge and component markup into single-line calls[cite: 1].
- **Automatic `<picture>` Expansion:** Any image URL containing a `@theme` token or `{live}` flag compiles into a responsive `<picture>` element automatically[cite: 1].
- **Local Asset Caching (`images/mdr/`):** External images are downloaded, content-hashed, and served locally from `images/mdr/`, with automatic pruning of unused assets and graceful fallback to direct URLs on network failure.
- **Live Widget Snapshots (`{live}`):** Dynamic stats cards keep their remote URLs in `<source>` while storing a local fallback snapshot in `<img>`.
- **Zero Dependencies:** Built strictly on the Rust standard library (`std`) for sub-second CI compilation.

---

### Quick Start

```bash
# Build and run from source
cargo run --release -- README.mdr README.md

# Or install locally via Cargo
cargo install --path .
{BIN_NAME} README.mdr README.md

```

**CLI Flags**

| Flag | Description |
| --- | --- |
| `--images-dir <PATH>` | Directory for managed assets (default: `<output_dir>/images/mdr`) |
| `--images-prefix <PREFIX>` | URL prefix emitted in compiled Markdown (default: `images/mdr`) |
| `--refresh-images` | Re-download all external images and `{live}` fallback snapshots |
| `--no-fetch` | Skip network requests and use cached/expected local paths |

---

### Syntax Overview

```mdr
@theme card = 161B22 | F6F8FA
@theme accent = 14B8A6 | 0F766E

@def badge(alt, label, logo) = ![{alt}](https://img.shields.io/badge/{label}-{card}?style=for-the-badge&logo={logo}&logoColor={accent})

# Static themed badge (cached in images/{BIN_NAME}/)
@badge(Rust, Rust, rust)
@var username = <username>

# Live remote image with local fallback snapshot
[![Stats](https://github-readme-stats.vercel.app/api?username={username}&title_color={accent}){height=175 live}](https://github.com/{username})
```

---

### Roadmap

* [x] `@theme` dark/light palette tokens and `@def` parameterized macros
* [x] Automatic `<picture>` expansion and `{live}` local fallback snapshots
* [x] Isolated `images/mdr/` asset caching, pruning, and offline fallback warnings
* [x] CLI output path inference (`mdr <file>.mdr`) and composite GitHub Action (`action.yml`)
* [x] Unified `Value` model with `@var` constants and `@list` tuple declarations
* [ ] Nested `@repeat` loops and resilient `@expand` windowing blocks
* [ ] `@include` composition for `.mdr` partials and local/remote `.json` feeds
* [ ] Multi-platform GitHub Releases and Homebrew tap (`brew install Powerisinschool/tap/mdr`)
