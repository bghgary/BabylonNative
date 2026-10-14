# Playground visual-test host

On Win32, `--playwright` starts a hidden Playground process with the standard-input/output protocol consumed by Babylon.js's `NativePage`. It supports JavaScript evaluation, framebuffer resize and screenshot export. It does not run the legacy validation script.

For the shared per-scene suite, build Playground with Release, V8 and D3D11, then set `BABYLON_NATIVE_PLAYGROUND` to the resulting executable. Use the shared `visualization.native.test.ts`, its original reference images and its normal comparison tolerances. Runtime packages must match the committed `Apps/package-lock.json`.

The desktop host is reused from the earlier Windows prototype. This branch pairs it with the native source and runtime used by the mobile measurements; it does not enable a production test migration.
