# Design QA — VersionPilot

Source visual truth: `/var/folders/9f/8771ld3s12qgy1xzkk2ks0wh0000gn/T/codex-clipboard-9c6fa899-5613-4e1c-b013-1e7426ca6048.png`

Implementation target: VersionPilot desktop bundle, 1120 × 760 px fixed window.

State: empty workspace and project workspace; light theme.

## Comparison notes

The implementation intentionally borrows the source’s high-level structure only: a quiet pale navigation rail, rounded/soft control language, compact label typography, a low-contrast workspace, and restrained blue primary actions. VersionPilot retains its version-management-specific inputs and scan configuration rather than reproducing the source dashboard’s content.

## Required fidelity surfaces

- Fonts and typography: uses the platform system font stack with compact 10–15 px control labels and a larger workspace title.
- Spacing and layout rhythm: fixed 1120 × 760 frame, 238 px navigation rail, 42 px workspace padding, 7–10 px control radii, and 24 px section rhythm.
- Colors and visual tokens: white/pale-gray surfaces, cool gray outlines, blue primary actions, and muted green/amber status colors.
- Image quality and asset fidelity: no source images are copied; the reference is used only for structural and visual direction. Standard UI icons use Phosphor Icons.
- Copy and content: all visible copy remains VersionPilot-specific.

## Findings

- [P1] Visual capture unavailable for the rebuilt bundle.
  Evidence: the desktop automation surface continues to resolve an older `com.versionpilot.app` bundle instead of the rebuilt `com.versionpilot.desktop` app, so a current implementation screenshot cannot be paired with the reference for a faithful pixel-level review.
  Fix: reopen the current bundle in a fresh desktop automation session, capture empty and project states at 1120 × 760, then compare them against the source image.

## Implementation checklist

- Rebuilt the desktop bundle with the screenshot-inspired sidebar shell and content panels.
- Added the in-app path modal so opening a project does not rely only on the native picker.
- Set a fixed 1120 × 760 window frame.

final result: blocked
