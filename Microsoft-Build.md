# AivoRelay Microsoft Store Build

This is the Microsoft Store version of AivoRelay.

## Differences from standard version

| Feature | Store Version | Standard Version |
|---------|--------------|------------------|
| Installation | Microsoft Store | MSI Installer |
| Updates | Automatic (via Store) | Built-in Updater |
| Environment | Sandboxed | Standard Application |

## Updates

The Microsoft Store version is updated automatically through the Microsoft Store. The built-in Tauri update system is disabled in this version.

## Troubleshooting

If you encounter issues specific to the Store version, please report them on GitHub.

## How to identify Store version:
- Window title: "AivoRelay (Store Edition)"
- Footer shows version with "(Microsoft Store Edition)" suffix, e.g., "v0.7.9 (Microsoft Store Edition)"

## Common Pitfalls (How to Break the Build)

Do **NOT** do these things, or the build will fail:

1.  **Setting `targets: ["msix"]` in `tauri.conf.json`**:
    Tauri 2 (in our current configuration) will report this as an invalid target. Please use `msi`. Microsoft Store packaging is handled through a separate process.

2.  **Mangling JSX in `Footer.tsx`**:
    When modifying the footer to hide the `UpdateChecker`, ensure all `<div>` tags are properly balanced. Incorrect JSX structure will cause TypeScript compilation errors.

3.  **Renaming the version in configuration files**:
    GitHub Actions require a strict SemVer format. Do not add suffixes like `-Store` to the version in `tauri.conf.json` or `package.json`. Keep the "Store Edition" suffix in the UI (React components) only to ensure the release workflow functions correctly.

## Release Process

To create a release for the Microsoft Store:

1.  Go to the **Actions** tab in your GitHub repository.
2.  Select the **Microsoft Store Release** workflow from the sidebar.
3.  Click **Run workflow** and select the `Microsoft-store` branch.
    - **Important**: The branch search is **case-sensitive**. Type `Mi` or `Microsoft` (with capital M), not `mi`.
4.  This will create a draft release with the tag `vX.X.X-store` and assets named `aivorelay-store_*.msi`.
5.  Check the draft release and publish it when ready.

### GitHub Actions: workflow_dispatch Visibility

For the "Run workflow" button to appear in the GitHub Actions UI:

1.  The workflow file (`.github/workflows/microsoft-store-release.yml`) **must exist in the default branch** (`main`).
2.  Once it's in `main`, you can select any other branch (like `Microsoft-store`) when running it.
3.  If you create a new workflow only in a feature branch, it won't show up in the UI until merged to `main`.

This specialized workflow ensures that:
- The tag is distinct from standard releases (`-store` suffix).
- The release title clearly indicates the **Microsoft Store Edition**.
- Artifacts use the `aivorelay-store` prefix to avoid confusion with standard binaries.
