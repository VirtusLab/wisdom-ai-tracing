# Homebrew Distribution

## Setup

### 1. Create the tap repository

Create a public repo `softwaremill/homebrew-tracevault` on GitHub with:

```
Formula/
  tracevault.rb    # copy from this directory
.github/
  workflows/
    update-formula.yml  # see below
```

### 2. Create the update-formula workflow

Add this to `softwaremill/homebrew-tracevault/.github/workflows/update-formula.yml`:

```yaml
name: Update Formula

on:
  repository_dispatch:
    types: [update-formula]

permissions:
  contents: write

jobs:
  update:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Update formula
        run: |
          VERSION="${{ github.event.client_payload.version }}"
          TAG="${{ github.event.client_payload.tag }}"
          SHA_AARCH64_DARWIN="${{ github.event.client_payload.sha256_aarch64_apple_darwin }}"
          SHA_X86_64_DARWIN="${{ github.event.client_payload.sha256_x86_64_apple_darwin }}"
          SHA_X86_64_LINUX="${{ github.event.client_payload.sha256_x86_64_unknown_linux_gnu }}"
          SHA_AARCH64_LINUX="${{ github.event.client_payload.sha256_aarch64_unknown_linux_gnu }}"

          cat > Formula/tracevault.rb << FORMULA
          class Tracevault < Formula
            desc "CLI tool for AI code tracing and attribution"
            homepage "https://github.com/VirtusLab/visdom-ai-tracing"
            version "${VERSION}"
            license "Apache-2.0"

            on_macos do
              on_arm do
                url "https://github.com/VirtusLab/visdom-ai-tracing/releases/download/${TAG}/tracevault-${TAG}-aarch64-apple-darwin.tar.gz"
                sha256 "${SHA_AARCH64_DARWIN}"
              end
              on_intel do
                url "https://github.com/VirtusLab/visdom-ai-tracing/releases/download/${TAG}/tracevault-${TAG}-x86_64-apple-darwin.tar.gz"
                sha256 "${SHA_X86_64_DARWIN}"
              end
            end

            on_linux do
              on_arm do
                url "https://github.com/VirtusLab/visdom-ai-tracing/releases/download/${TAG}/tracevault-${TAG}-aarch64-unknown-linux-gnu.tar.gz"
                sha256 "${SHA_AARCH64_LINUX}"
              end
              on_intel do
                url "https://github.com/VirtusLab/visdom-ai-tracing/releases/download/${TAG}/tracevault-${TAG}-x86_64-unknown-linux-gnu.tar.gz"
                sha256 "${SHA_X86_64_LINUX}"
              end
            end

            def install
              bin.install "tracevault"
            end

            test do
              assert_match version.to_s, shell_output("#{bin}/tracevault --version")
            end
          end
          FORMULA

      - name: Commit and push
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          git add Formula/tracevault.rb
          git commit -m "Update tracevault to ${{ github.event.client_payload.version }}"
          git push
```

### 3. Create a GitHub PAT

Create a fine-grained personal access token with:
- **Repository access**: `softwaremill/homebrew-tracevault`
- **Permissions**: Contents (read/write)

Add it as a secret named `HOMEBREW_TAP_TOKEN` in the `VirtusLab/visdom-ai-tracing` repository.

### 4. Usage

After a release is published (via release-plz), the `release-binaries` workflow:
1. Builds binaries for all 4 targets
2. Uploads them as release assets
3. Dispatches an event to the tap repo to update the formula

Users install with:

```bash
brew install softwaremill/tracevault/tracevault
```

Or add the tap first:

```bash
brew tap softwaremill/tracevault
brew install tracevault
```
