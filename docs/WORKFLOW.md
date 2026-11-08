# 🛠️ Software Engineering & Git Flow Workflow

This document outlines the professional engineering lifecycle for **Switch Extractor**, covering branching strategies, GitHub Projects (Kanban), pull request conventions, automated CI verification, and production release flows.

---

## 📌 1. Git Branching Strategy

We follow a robust **Git Flow / Trunk-Based Integration** model:

```
[ feature/xyz ]  ──(PR + CI)──►  [ develop ]  ──(Release PR)──►  [ main ]
                                (Integration)                     (v1.0.x Release)
```

| Branch | Purpose | Protection |
|---|---|---|
| **`main`** | **Production Releases**. Every merge into `main` automatically cuts a new version tag (e.g. `v1.0.1`) and publishes a GitHub Release with standalone binaries and checksums. | Protected (merge only from `develop`) |
| **`develop`** | **Active Integration Branch**. All feature branches and bug fixes are merged here after passing CI checks. | Protected (requires green CI checks) |
| **`feature/*`** | Feature branches for new capabilities (e.g. `feature/dlc-batch-extract`, `feature/xci-repacker`). | Merges into `develop` |
| **`fix/*`** | Bug fix branches (e.g. `fix/bktr-block-align`, `fix/nca-header-key`). | Merges into `develop` |

---

## 📋 2. GitHub Projects (Kanban Board) Setup

To organize work items professionally:

1. Navigate to your repository on GitHub.
2. Click **Projects** in the top navigation tab.
3. Click **New Project** &rarr; select **Board (Kanban)** template.
4. Name the board: `Switch Extractor Engineering Board`.
5. Configure the columns:
   - 📥 **Backlog**: Ideas, format investigations, and user-submitted requests.
   - 🎯 **Todo**: Tasks prioritized for the current milestone.
   - ⏳ **In Progress**: Actively being developed on a dedicated branch.
   - 🔍 **In Review**: Pull Request opened, awaiting CI checks & review.
   - ✅ **Done**: Merged into `develop` or deployed to `main`.

---

## 🔄 3. Step-by-Step Task Development Lifecycle

### Step 1: Create or Pick a Task
1. In the GitHub Kanban board, create a new Issue using the templates:
   - **Bug Report** (`.github/ISSUE_TEMPLATE/bug_report.yml`)
   - **Feature Request** (`.github/ISSUE_TEMPLATE/feature_request.yml`)
   - **Engineering Task** (`.github/ISSUE_TEMPLATE/task.yml`)
2. Move the issue to **Todo**, assign yourself, and note the issue number (e.g. `#12`).

### Step 2: Branch from `develop`
```powershell
git checkout develop
git pull origin develop
git checkout -b feature/12-add-bktr-relocation
```

### Step 3: Implement & Test Locally
Run the automated test suite and verify the build before committing:
```powershell
# 1. Run core cryptographic and format unit tests
cargo test --manifest-path crates/switch-extractor-core/Cargo.toml

# 2. Verify frontend compilation
npm run build

# 3. Test standalone compilation
npm run tauri build -- --no-bundle
```

### Step 4: Commit with Conventional Commits
```powershell
git commit -m "feat(bktr): implement bucket tree relocation and subsection decryptor"
```
Prefix conventions:
- `feat(...)`: New feature or user-facing capability.
- `fix(...)`: Bug fix or patch.
- `perf(...)`: Performance optimization (streaming I/O, AES-NI).
- `refactor(...)`: Code cleanup without functional change.
- `test(...)`: Adding or updating test cases.
- `chore(...)`: Workflow, dependencies, or configuration updates.

### Step 5: Push and Open a Pull Request
1. Push your branch:
   ```powershell
   git push -u origin feature/12-add-bktr-relocation
   ```
2. Open a Pull Request targeting **`develop`**.
3. Fill out the PR template (`PULL_REQUEST_TEMPLATE.md`), linking `Closes #12`.
4. GitHub Actions CI will automatically run tests and verify the build.

### Step 6: Merge to `develop`
Once CI checks pass green, squash-and-merge or rebase-merge into `develop`.

---

## 🚀 4. Automated Release Process

When a milestone is ready for release:
1. Open a Pull Request from `develop` into `main`.
2. Once merged into `main`:
   - The **Build & Release** workflow triggers automatically.
   - It fetches existing semantic version tags (`v1.0.0`, `v1.0.1`, etc.).
   - It auto-increments the patch version (or follows explicit tag if triggered by tag push).
   - Runs full test suite (`cargo test`).
   - Builds standalone `Switch_Extractor.exe` with Tauri v2 and MSVC.
   - Packages `Switch_Extractor-windows-x64.zip` with `README.md` and `LICENSE`.
   - Generates `SHA256SUMS.txt`.
   - Creates a GitHub Release with full release notes and attached assets.
