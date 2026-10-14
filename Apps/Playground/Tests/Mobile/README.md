# Native mobile visual tests

This opt-in runner builds the Playground for Android or iOS, uploads it to BrowserStack App Automate, and drives the existing `Scripts/validation_native.js` catalog. It uses native framebuffer pixels, not operating-system screenshots. It does not change the committed references, comparison thresholds, render counts, catalog exclusions, or normal Playground startup.

## Build and run

Android requires the normal Playground toolchain. From `Apps/Playground/Android`, add `-PmobileVisualTests` to the ARM64 Release build:

```powershell
.\gradlew.bat 'assembleRelease' '-PjsEngine=V8' '-PARM64Only' '-PNDK_VERSION=28.2.13676358' '-PmobileVisualTests' '-PBABYLON_NATIVE_PLUGIN_NATIVEDRACO=ON' '-PBABYLON_NATIVE_PLUGIN_NATIVEMESHOPT=ON' '-PSANITIZERS=OFF'
```

iOS requires macOS and the repository's supported Xcode toolchain. Configure with `BABYLON_NATIVE_PLAYGROUND_MOBILE_TESTS=ON` and `BABYLON_NATIVE_PLUGIN_TESTUTILS=ON`, build Playground for `iphoneos` with `CODE_SIGNING_ALLOWED=NO`, and zip `Payload/Playground.app` into an IPA. BrowserStack re-signs the uploaded IPA; this is not an App Store package. The manual workflow contains the complete build and archive steps.

Both app variants package the same native catalog, reference images, fonts and Babylon.js version as the checkout. Without a mobile launch endpoint they load the ordinary Playground experience. The transport is omitted from default builds. Never distribute the test-enabled app as a production app: it deliberately accepts JavaScript commands from its private controller.

The iOS test build permits HTTP only for the exact `bs-local.com` controller domain. Ordinary iOS builds retain their existing App Transport Security policy; subdomains and global arbitrary loads are not enabled.

From this directory, install Node.js 22 or later dependencies with `npm ci`, run protocol/image controls with `npm test`, and run device tests with `npm run visual`. Set these environment variables:

| Variable | Meaning |
| --- | --- |
| `BROWSERSTACK_USERNAME`, `BROWSERSTACK_ACCESS_KEY` | Credentials supplied by the approved account owner/secret store. |
| `MOBILE_APP` | Absolute path to the APK or unsigned IPA from this checkout. |
| `MOBILE_PLATFORM` | `android` or `ios`. |
| `MOBILE_DEVICE`, `MOBILE_OS_VERSION` | Exact pair in the live App Automate inventory; no nearest-device substitution. |
| `BROWSERSTACK_LOCAL_BINARY` | Installed BrowserStack Local executable; defaults to the user's `.browserstack` directory. |
| `MOBILE_TEST_INDICES` | Optional nonempty JSON array of native catalog indices, such as `[131,149]`. Excluded scenes are rejected. |
| `MOBILE_SHARD` | Optional `n/total`, default `1/1`. Partitions by original catalog index; at most five shards. |
| `MOBILE_OUTPUT` | Optional result directory; default `mobile-results` beside this file. |

Use the same source checkout for the app and controller. Runtime Babylon.js version is checked against `Apps/package-lock.json`. Current account capacity and exact device availability are checked before allocation. Each runner uses one slot; externally coordinated runners must leave capacity for other users.

## Completion and diagnostics

Each scene uses a fresh NativeEngine. Commands and scene events are correlated, frame/result delivery is acknowledged before moving on, and PNG output is lossless RGBA with independently checked dimensions and checksums. The app's explicit native-validation exit code must agree with scene outcomes. There is no screenshot polling, artificial delay per frame, or success inferred from inactivity.

`results.json` records selection, exclusions, individual outcomes, package hash, device/runtime information, startup timing, device occupancy and cleanup. Its `complete` field means every selected scene reached an outcome, not that every scene passed; cleanup failures are recorded separately. Rendered PNGs accompany it; mismatches also retain a comparison PNG with differing reference pixels marked red. Any mismatch, timeout, missing completion, missing passing-scene frame or infrastructure failure fails the Playwright test. An unresponsive scene is recorded as a failure; its app/session is discarded and the remaining scenes continue in a replacement session. The timed-out scene is never silently retried. References are never updated by this runner.

The controller binds only to loopback and uses a random endpoint per session. BrowserStack Local must both start locally and register the matching active cloud identifier before allocation. Account credentials stay on the host, never in app launch arguments or scripts. Traces, video, network capture and Appium debug screenshots are disabled. Private Local logs live outside result artifacts and are deleted during cleanup; a deletion failure cannot include them in the uploaded result directory. Native device diagnostics are filtered and credential-redacted before saving.

## Manual continuous integration

`.github/workflows/mobile-visual-tests.yml` has only a manual trigger. Its default checks the portable runner, then builds and retains APK/IPA artifacts without starting paid sessions. Existing pull-request checks are unchanged.

Before selecting `run-cloud`, the account owner must confirm approved credentials, rotation status, plan/cost and permitted devices. Configure the `browserstack-mobile` environment with required reviewers and allowed branches, its two credential secrets, and `MOBILE_TESTING_APPROVED=true`. Do not put credentials in workflow inputs, repository files, chat-derived recovery scripts, command-line URLs or result artifacts. Run only trusted revisions: the selected native scenes and test code execute while credentials are available.

The optional cloud jobs use one device per selected platform, at most two simultaneous sessions. Additional device/operating-system combinations require separate explicit runs and measured coverage; one device is not platform-wide qualification. This workflow is not a production rollout or a purchase authorization.
