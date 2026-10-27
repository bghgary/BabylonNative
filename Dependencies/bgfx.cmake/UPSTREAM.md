# bgfx.cmake provenance

This wrapper was imported from https://github.com/BabylonJS/bgfx.cmake at
`8fd48e454f624f964ced930af597fa5b4526be18`. Its 53 blob files were initially
imported byte-for-byte with their original modes. Its CC0 dedication remains
unchanged in [LICENSE](LICENSE); the original attribution headers are retained.

All 706 reachable commits were rewritten under `Dependencies/bgfx.cmake` and
merged without squashing. Authors, author/committer dates, messages, historical
renames, empty commits, and merge topology are preserved. Every historical
mode-160000 gitlink was removed; no bgfx, bx, or bimg implementation or history
was imported. The rewritten tip is
`39f003f6ae0b6e92b1c3bd0152f164c0e56000b2`.
Path rewriting necessarily changes commit IDs and invalidates old signatures.
[UPSTREAM_COMMIT_MAP.txt](UPSTREAM_COMMIT_MAP.txt) maps all original commit IDs
to their imported IDs. BabylonNative's existing history was not rewritten.

The former gitlinks now correspond to pinned CMake source acquisitions:

| Source | Repository | Revision |
|---|---|---|
| bgfx | https://github.com/BabylonJS/bgfx | `5068996b4161522bc7d8cd1c7fb6b95e11771006` |
| bx | https://github.com/bkaradzic/bx | `f3a9adb603c5677731e7cf10622fa16e1dfe9ea5` |
| bimg | https://github.com/bkaradzic/bimg | `9bfbd6493b48a15238f2d1374a718ec026524515` |

## Current wrapper

Apart from this file, `UPSTREAM_COMMIT_MAP.txt` and the excluded `.github`
directory, the wrapper matches https://github.com/bkaradzic/bgfx.cmake at
`5ae5fa5b121974b9de084539bb784d97fa4cd8a6` plus the changes intended for
upstream: FetchContent names `bx`, `bimg` and `bgfx`, no nonexistent source
subdirectory overrides or redundant `GIT_SHALLOW FALSE` settings, and
corresponding README updates. The wrapper's
fallback pins match that upstream revision; BabylonNative's root declarations
retain the separate revisions listed above.

This source update does not import or rewrite history. The original import tip
and commit map above remain unchanged.

## BabylonNative integration

BabylonNative-specific source selection and target settings live outside this
directory:

- The repository root `CMakeLists.txt` declares `bx`, `bimg` and
  `bgfx` with BabylonNative's pins and standard source directories under
  `${FETCHCONTENT_BASE_DIR}/{bgfx,bx,bimg}-src`. FetchContent's first
  declaration wins, so the wrapper selects those sources instead of its fallback
  pins. `FETCHCONTENT_SOURCE_DIR_BGFX`, `_BX` and `_BIMG` overrides select
  those sources directly.
- `Dependencies/CMakeLists.txt` sets wrapper options, adds this directory with
  `add_subdirectory` and adjusts the resulting targets.

Source packages must include this tracked wrapper and the selected child source
trees separately; consumers supply the child source overrides for offline builds.

`BGFX_DIR`, `BX_DIR` and `BIMG_DIR` keep the upstream wrapper's precedence and
semantics described in [README.md](README.md).

The upstream `.github` directory is excluded from the vendored working tree in
a separate BabylonNative-only commit. All five files and their histories remain
in the imported ancestry. This exclusion is not an upstream wrapper change;
never export it as a deletion of upstream's automation.

## Updates

For a future wrapper update, fetch the original full history into a disposable
clone, use the same path/gitlink transformation without pruning commits, and
reuse the mapped import ancestry before merging new wrapper changes. Keep
adaptations separate and extend the map and verification for the new tip.
Update BabylonNative's child pins only deliberately in the repository root
`CMakeLists.txt`; CMake
updates managed sources on reconfigure but never checks out explicit roots.

## Exporting wrapper changes upstream

Prepare the wrapper changes in a standalone checkout of the current upstream
revision above. Keep its wrapper files identical to this directory, excluding
the BabylonNative provenance files, and preserve upstream's `.github` directory.
The dependency-name change also requires updating the fetched-source paths in
upstream's release workflow.

Compare against that upstream revision, not the original import tip: changes
already accepted upstream must not be proposed again. Keep BabylonNative's root
source pins, target settings and package layout out of the upstream patch.

**Do not squash this import when landing.** Upstream currently permits only
squash merges. Landing requires a maintainer-approved history-preserving
mechanism; a normal squash merge discards the imported ancestry.
