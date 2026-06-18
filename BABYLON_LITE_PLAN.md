# Babylon Lite on Babylon Native — Options & Plan

> **Status:** Draft for team review. Iterate in-place.
> **Audience:** Babylon Native + Babylon Lite teams.
> **Framing (post-meeting, 2026-04-30):** The goal of this doc is *"don't preclude Babylon Native"*, **not** "implement Babylon Native for Babylon Lite now." But "don't preclude" can't be answered on paper — we need **Phase-0 prototypes** (§8) to produce real data on binary size, pixel parity, perf, and integration cost. Without those prototypes the eventual plan is opinion. The concrete deliverable from this doc is therefore a list of investigation **and prototype** tickets that drive toward a solid plan.
> **Constraints:**
> - Changes to Babylon Lite are **on the table** — it is a young, single-engine codebase and we can influence its design.
> - **Frame graph is no longer hypothetical.** It is being landed in [Babylon-Lite PR #63](https://github.com/BabylonJS/Babylon-Lite/pull/63) (Popov72, open as of writing). This doc treats the FG as it actually exists in that PR.

---

## 1. Document history

- **v1:** Framed the integration as "WebGPU polyfill (Dawn / wgpu) vs. C++ port." Too narrow.
- **v2:** Discovered Lite's existing internal seams (`RenderingContext`, `Renderable`, `MeshGroupBuilder`); added "change Lite" as a first-class axis; added the "what problem are we solving?" goal-ranking question (G1/G2/G3).
- **v3 / v3.1:** Added the (then-hypothetical) frame graph as one candidate seam.
- **v4 (this version):** Folds in (a) the cross-team meeting on 2026-04-30 (full notes in §A) and (b) the actual frame-graph design landing in [Babylon-Lite PR #63](https://github.com/BabylonJS/Babylon-Lite/pull/63) (full breakdown in §B). The biggest changes:
  - Reframed deliverable: **"don't preclude" as the outcome, prototyping as the means.** Not "implement now," but also not "decide on paper." Phase-0 prototypes feed the real plan.
  - **§2 rewritten** against the real FG code on the PR branch (no more "assumed").
  - **§4 S0a split** into S0a-narrow (orchestration only — low-value) and S0a-wide (native `RenderPassTask` — the real prize). Added **S0d** (Sergio's middle-seam: custom JS↔C wire, not strictly FG-aligned).
  - **`Renderable` / `DrawBinding` promoted to co-equal seam with `Task`** — you can't replace one without the other.
  - **U2′ marked partly obsolete** (FG is already pluggable in spirit). Added **U7** (remove WebGPU types from Lite public API — DK already pushing on PR #63), **U8** (optional `keepCPUCopy` on GPU buffers), **U9** (pass encoder to `Task.execute(encoder)` instead of reading `engine._currentEncoder`).
  - New **§5a Custom-task authoring story** (Ryan's concern: external users writing tasks for N platforms).
  - New **§5b Profiling baseline** (Ryan/Sebastien gap: nobody actually knows what's expensive in non-JIT JS — this becomes Phase-0 spike #1).
  - Phasing reordered to put profiling first.

---

## 2. What Lite actually looks like inside (current master + frame-graph PR #63)

I read the source on `master @ 3050ad4` (~21k LOC of TS) and the FG branch `feature/pr-63-frame-graph-rendering` (PR #63, currently open, +2912 / −2672, 106 files). What follows describes Lite *as it will be* once #63 merges — that's the substrate the integration plan needs to target.

### The frame graph (as implemented in PR #63)

Three small files plus one big one — see §B for a deeper breakdown.

- **`frame-graph/task.ts` — `Task` interface** (42 lines):
  ```ts
  interface Task {
      readonly name: string;
      readonly engine: EngineContextInternal;
      readonly scene: SceneContextInternal;
      record(): Promise<void> | void;   // build phase, may be async
      execute(): number;                // per frame; returns draw count
      dispose(): void;
  }
  ```
  Encoder is read via `engine._currentEncoder` (global), **not** passed as a parameter. `record()` is called once at `frameGraph.build()` time; `execute()` is called once per frame.

- **`frame-graph/frame-graph.ts` — `FrameGraph`** (88 lines):
  An ordered `Task[]` with `build()` / `execute()` / `dispose()`. Explicit comment in source: *"There is no privileged 'main' task: a scene-render task that draws into the swapchain is just one task among many."* Order is the user's responsibility (`addTask` / `addTaskAtStart` / `addTaskBefore`).

- **`frame-graph/render-pass-task.ts` — `RenderPassTask`** (519 lines, the substance):
  - Single execute path for both swapchain and offscreen render targets.
  - Renderable population: explicit (`addToPass(mesh, opts)`) or auto-mirror from `scene._renderables` with re-sync on `_renderableVersion`.
  - Buckets opaque / transmissive / transparent; per-frame back-to-front sort on transparent.
  - **Caches the opaque draws as a `GPURenderBundle`**, invalidated by mutation/visibility version.
  - Per-pass camera override (`cam`) and clear control (`clr`/`clrColor` — `"load"` mode for overlay passes).
  - **Each pass owns its own scene UBO + bind group 0** (avoids cross-pass state corruption when several passes share a frame with different cameras).
  - Calls WebGPU directly: `device.createRenderBundleEncoder`, `encoder.beginRenderPass`, `pass.setBindGroup`, `pass.executeBundles`, `pass.setViewport`, `pass.setScissorRect`, `pass.end`.

- **`engine/render-target.ts` — `RenderTarget`**: pure-state descriptor + GPU textures allocated at build time. Swapchain mode shares the engine's MSAA + depth textures; the swap view is acquired per-frame and patched into the descriptor at execute.

### Scene wiring

`createSceneContext` eagerly creates a `_frameGraph` with one default `RenderPassTask` that mirrors `scene._renderables` to the swapchain. `scene._record()` calls `_frameGraph.execute()`. Resize triggers `void _frameGraph.build()`. **Rendering today goes through the FG; there is no non-FG path.** Adoption-cost is therefore zero.

### Public API (post-PR-63 `index.ts`)

```ts
export { getFrameGraph } from "./scene/scene.js";
export type { FrameGraph } from "./frame-graph/frame-graph.js";
export { addTask, addTaskAtStart, addTaskBefore } from "./frame-graph/frame-graph-actions.js";
export type { Task } from "./frame-graph/task.js";
export type { RenderPassTask, RenderPassTaskConfig } from "./frame-graph/render-pass-task.js";
export { createRenderPassTask, removeMeshFromTask } from "./frame-graph/render-pass-task.js";
export type { RenderTarget, RenderTargetDescriptor } from "./engine/render-target.js";
export { createRenderTarget } from "./engine/render-target.js";
export { createRenderTargetTexture } from "./texture/rtt.js";
```

### Layering once PR #63 lands

```
┌──────────────────────────────────────────────────────────────────────┐
│ Public API (index.ts)                                                │
│   createEngine, createSceneContext, loadGltf, createPbrMaterial,    │
│   createRenderPassTask, createRenderTarget, addTask*, ...            │
├──────────────────────────────────────────────────────────────────────┤
│ Scene / Authoring descriptors (pure data — NO GPU coupling)          │
│   Mesh, Camera, LightBase, Material, Texture2D, SceneContext,        │
│   shadow configs, asset containers, glTF/env/HDR/KTX/Basis loaders   │
├──────────────────────────────────────────────────────────────────────┤
│ FrameGraph  (ordered Task[]; no privileged "main" pass)              │
│   build() → record() each Task (allocate RTs, build descriptors)    │
│   execute() → drain Tasks into engine._currentEncoder                │
├──────────────────────────────────────────────────────────────────────┤
│ Task implementations  ← seam #1 for native (replace per-task)        │
│   RenderPassTask, EffectRenderTask, …                                │
│   Each Task.execute() encodes GPU work directly to WebGPU            │
├──────────────────────────────────────────────────────────────────────┤
│ MeshGroupBuilder + Renderable + DrawBinding  ← seam #2               │
│   Per-binding draw(): pass.setBindGroup(...); pass.drawIndexed(...)  │
│   Materials own shaders + pipelines                                  │
├──────────────────────────────────────────────────────────────────────┤
│ WebGPU (navigator.gpu, GPUDevice, GPUBuffer, GPUTexture, WGSL, ...)  │
└──────────────────────────────────────────────────────────────────────┘
```

### Key insight: the seam isn't the FrameGraph, it's the Task

The FG's `execute()` is a 5-line for-loop dispatching to polymorphic Tasks. There's no orchestration intelligence to replace at that layer. **The real native value comes from replacing `RenderPassTask` itself** (and any sibling task types) with native implementations — the FG just iterates them. This invalidates the v3 framing of "S0 = pluggable FG executor" and reshapes §4 (see S0a split below).

### A second seam shows up: Renderable / DrawBinding

Even a native `RenderPassTask` calls `r.bind(eng, sig)` and `binding.draw(encoder, engine)` per renderable. So replacing the Task seam is **not enough** unless `encoder` is abstracted. The two seams (Task and Renderable/DrawBinding) have to be designed together — see S0a-wide and U9 below.

### Existing seams from v2/v3 — status update

- **`RenderingContext`** still exists in `engine.ts` and is what scene registers as. Largely subsumed by the FG layer for our purposes.
- **Extension pillar** (GUIDANCE.md §4c′): unchanged. Friendly to a "backend" axis if we go that route.
- **Loaders are pure data**: unchanged.
- **Materials own shaders + pipelines** (pillar 4c): unchanged.
- **WebGPU types leak in the public API today** (e.g., `createRenderTargetTexture` exposes raw `GPUTexture`/`GPUTextureView`/`GPUSampler`). DK flagged this on PR #63; Alexis confirmed in the meeting it has to be removed. Tracked as **U7** (§5).

---

## 3. What problem are we solving?

The right answer depends heavily on the actual goal. Three candidate framings — we should pick one (or rank them) before picking an implementation.

- **G1. "Run Lite unmodified on native devices."**
  Primary driver: reuse Lite's JS/WGSL investment on native; future WebGPU JS content just works.
  Favors: real WebGPU in Babylon Native.

- **G2. "Get Lite's size/perf/simplicity story on native."**
  Primary driver: smallest-possible native Babylon runtime, WebGPU-modern pipeline, pixel-parity with Lite web.
  Favors: native renderer that shares Lite's descriptor shape; potentially Lite-as-authoring-DSL.

- **G3. "Bring Lite's loaders / material descriptors to native Babylon.js / Babylon Native consumers."**
  Primary driver: Lite already has clean glTF/env/HDR loaders and tight PBR descriptor types; these are useful to native apps that don't need the whole engine.
  Favors: hook at the descriptor layer; cheap.

These goals overlap but pull in different directions at implementation time. **Decision #1 the team needs to make** is which one (or which ranked combo) is the driver.

### What the meeting added

- The room (Branimir, Sebastien, Sergio) leaned **away from S1/Dawn-polyfill and toward "more in native than just FG"** — i.e., closer to G2 in flavor. No formal decision; just direction.
- **Sebastien:** rendering may not be the dominant cost in non-JIT JS. Physics / picking / collision / `evaluateActiveMesh`-style work may dominate. Implication: scope shouldn't be "rendering seam only."
- **Sergio:** the FG layer wasn't designed with a native split in mind. We could design a **custom JS↔C wire** sitting between Babylon-API level and FG level — shared memory buffer for state, expensive APIs done in C — without strictly riding on FG's existing surface. (Captured as **S0d** in §4.)
- **Gary's reframing (now top-of-doc):** the goal of *this* document is not to ship Babylon Native for Lite, but to ensure Lite's design (especially the FG) doesn't preclude a native path later. Confirming "doesn't preclude" requires **prototyping** — we cannot resolve binary size, parity, perf, or integration cost on paper. So the output of this doc is **investigation + prototype tickets**, not architecture commitments.

---

## 4. Option space, organized by seam

Options are grouped by **where** in Lite's stack we intercept. Each option lists who owns what, upstream Lite changes required, and trade-offs. "Upstream Lite changes" ranges from *none* (seam already supports it) to *significant* (breaks or redefines a core pillar).

Legend: **🧩** = requires Lite upstream change, **🌐** = requires native WebGPU-capable surface, **🏎️** = native renderer required, **📦** = descriptor-only reuse.

### S0 — Hook at / above the Frame Graph layer

**Major v4 update:** with PR #63 visible, the FG itself is essentially a polymorphic dispatcher (5-line for-loop). There is no orchestration intelligence at the FG layer to replace. The leverage is in the **Task implementations** (especially `RenderPassTask`), and possibly in a **purpose-built JS↔C wire** above the FG. S0a is therefore split.

- **S0a-narrow. Native FG executor only (Tasks still WebGPU).** ❌ low-value
  Keep `Task` implementations as they are (WebGPU API calls); ship a native version of `FrameGraph.execute()` that iterates Tasks. Effectively useless: orchestration cost is negligible, all the GPU work still runs through WebGPU.

- **S0a-wide. Native `RenderPassTask` (and siblings) — the real prize.** 🌟
  The published `Task` interface (`record`/`execute`/`dispose`) is a tidy plug point. Provide a native implementation of `createRenderPassTask` that records to Dawn/Metal/D3D directly instead of WebGPU. The FG just iterates Tasks; replacing one Task type does the work.
  - *Pros:* Smallest possible native surface. FG-level scene/material/loader code unchanged. Render-bundle caching, per-pass scene UBO, pass ordering all inherited "for free." Per-pass camera/clear isolation already in #63 is exactly what a native implementation needs.
  - *Cons:* `Task.execute()` reads `engine._currentEncoder` (a WebGPU-typed global). For native, we either swap the encoder type (per-engine-mode) or pass it as a parameter — see **U9**. Also: `RenderPassTask` calls `r.bind(eng, sig)` per renderable, returning a `DrawBinding` whose `draw(encoder, engine)` calls WebGPU directly — so this seam **must be paired with seam #2** (Renderable / DrawBinding, see S3 below).
  - *Lite upstream asks (§5):* **U7** (no WebGPU types in public API), **U9** (encoder as parameter to `Task.execute`).

- **S0b. FG above Dawn (hybrid).**
  Keep WebGPU as the underlying API; bring Dawn (S1a) into Babylon Native. FG and all Tasks unchanged.
  - *Pros:* Lite + any WebGPU content runs unmodified at every layer. Highest pixel-parity confidence. Strategically enables full-Babylon WebGPU-on-native too.
  - *Cons:* Pays Dawn's binary cost (~tens of MB). No Lite-specific wins.

- **S0c. Record-to-stream FG executor (no JS-side GPU).**
  JS-side FG emits passes + resources + per-pass draw lists as plain data; native C++ consumes the stream. Requires `DrawContext` abstraction at the renderable layer (U3).
  - *Pros:* No `GPUDevice` in JS; preserves "WebGPU exclusive on web" pillar.
  - *Cons:* Effectively re-implements FG state on the native side. Significant cross-language design.

- **S0d. Custom JS↔C wire above the FG (Sergio's middle-seam).** 🆕 v4
  Don't ride strictly on the FG's Task interface — design a **purpose-built JS↔C contract** sitting between Babylon-API level and FG level. JS sets state into a **shared memory buffer** (lights, transforms, scene-level state); native consumes the buffer and runs the heavy work (sorting, culling, scene traversal, frame-graph orchestration).
  - *Pros:* Doesn't constrain the seam to Task-shaped chunks. Can move beyond rendering — physics/picking/`evaluateActiveMesh` become candidates too (Sebastien's point). One-call-per-frame JS↔native crossing minimizes the trip cost (Branimir's main argument).
  - *Cons:* New surface to design and maintain. Risk of duplicating FG state. Less "drop-in" than S0a-wide.
  - *Why it's worth keeping on the table:* the meeting consensus was that rendering is probably *not* the only thing worth pushing native, and a Task-only seam might be too narrow.

### Why S0 reshapes the rest of the option space

If we end up choosing S0a-wide *and* the published `Task` boundary stays as it is in #63:
- **S2 (`RenderingContext`)** is fully subsumed — scene already drives FG which already calls Task.execute. There is no separate `RenderingContext` seam to design against; we replace Tasks instead.
- **S3 (`Renderable` / `MeshGroupBuilder` / `DrawBinding`)** is **co-equal** with S0a-wide, not subordinate. A native `RenderPassTask.execute()` calls `binding.draw(encoder, engine)` per renderable — so unless renderables also have native draw paths, we still cross to WebGPU. Treat these together. (See §6 trade-off table.)
- **S4 (WGSL/Tint)** remains the shader-translation question — orthogonal. Pairs with both S0a-wide and S2.
- **S5/U1** (host injection) is still always-recommended and independent of S0.

If we go S0d (Sergio's middle-seam) instead, S2/S3 are mostly bypassed; the design lives one layer above the FG.

If we do *not* choose S0 at all (e.g., S1a/Dawn or S7 C++ port), the existence of the FG in Lite is largely neutral to native — it changes Lite internals but Babylon Native still intercepts at WebGPU or below.

### S1 — Hook below WebGPU: polyfill `navigator.gpu`

Lite runs unmodified; Babylon Native provides a WebGPU implementation to the hosted JS engine.

- **S1a. Dawn (Google)** 🌐
  NAPI bindings to Chromium's reference WebGPU impl (Tint compiles WGSL to HLSL/MSL/SPIR-V). Covers D3D12/Metal/Vulkan/GL.
  *Pros:* Lite + any WebGPU JS works unmodified. Highest pixel-parity confidence (same Tint as Chrome). Strategically enables Babylon.js full WebGPU engine on native too.
  *Cons:* Heavy build (GN+CMake, SPIRV-Tools, Tint, abseil). Large binary. Mobile story less battle-tested. Coexists with — or eventually supersedes — bgfx inside Babylon Native.
- **S1b. wgpu-native (gfx-rs/Mozilla)** 🌐
  Same shape, Rust impl, single C ABI (`webgpu.h`).
  *Pros:* Simpler link (one static lib); good mobile support; Deno's choice.
  *Cons:* Rust toolchain required; WGSL validator diverges from Chrome → pixel-parity risk vs. Lite's goldens. Must measure.
- **S1c. Pluggable `webgpu.h` abstraction** 🌐
  Build the NAPI binding against standardized `webgpu-headers` so Dawn or wgpu-native slot in interchangeably. Adopted *inside* S1a or S1b.
  *Always-do variant* once we're on this path.
- **S1d. Reuse node-webgpu / Deno WebGPU bindings** 🌐
  Lift the JS-binding glue (promise handling, error scopes, IDL plumbing) instead of writing from scratch. Accelerator for S1a/S1b.
- **S1e. WebView2 / WKWebView as interim backend** 🌐
  Not a shipping answer, but a fast *bring-up / validation rig*: run the Lite `lab/` under a real browser WebGPU so we have a known-good reference while the native backend is under construction.
- **S1f. WebGPU → bgfx shim** ❌
  Rejected (v1 already): WGSL→bgfx-shader story breaks; WebGPU's explicit bind-group/command model does not map onto bgfx; pixel parity unlikely.
- **S1g. Record-and-replay fake `GPUDevice`**
  A TS "fake device" that records intent into a native-consumable command stream, then a C++ replayer executes against bgfx/native.
  *Pros:* No real WebGPU in-process.
  *Cons:* You end up implementing most of WebGPU's validation/semantics anyway (bind groups, layouts, error scopes, mapped buffers). Slides back toward S1a cost without S1a's strategic upside. Low confidence.

**Upstream Lite changes:** none for S1a–S1f. Recommended-anyway: §S5 host-injection hook.

### S2 — Hook at the engine's `RenderingContext` seam 🧩🏎️

Don't make scene register as a `RenderingContext`. Register *our own* context whose `_update`/`_record` iterates Lite's scene data and draws via a native renderer.

- **S2a. Native `RenderingContext` with a generic command abstraction.**
  Propose upstream: change `_update(encoder: GPUCommandEncoder)` / `_record(pass: GPURenderPassEncoder)` to accept an abstract `RecordContext` / `DrawContext`. Provide a WebGPU adapter (default) and a native adapter (Babylon Native).
  *Pros:* Keeps Lite's scene graph, loaders, shadow/picking/material pipeline descriptors intact. Replaces only the GPU issuance layer. No WebGPU runtime on native.
  *Cons:* **Breaks the "WebGPU Exclusive" pillar** (§1 of GUIDANCE.md) — requires an explicit carve-out or pillar reinterpretation ("WebGPU-shape exclusive; any backend that honors the shape is allowed"). Native renderer still has to reach pixel-parity against Lite's WGSL outputs (same problem as writing NativeEngine today, minus WebGPU JS).
  *Upstream Lite change:* abstract out WebGPU types from `RenderingContext` and `Renderable` interfaces. Significant but focused.

- **S2b. Two `RenderingContext` implementations coexisting.**
  Lite keeps WebGPU as default; Babylon Native ships its own context that reads Lite scene state directly and never invokes Lite's WebGPU paths.
  *Pros:* Less upstream surgery if we only expose the context protocol generically.
  *Cons:* Still requires genericizing the interface; otherwise we can't implement the context without faking a `GPUCommandEncoder`.

### S3 — Hook at the `Renderable` / `MeshGroupBuilder` / `DrawBinding` seam 🧩🏎️

**Co-equal seam with S0a-wide in v4.** PR #63's `RenderPassTask.execute()` calls `binding.draw(encoder, engine)` per renderable — so even a fully native Task still crosses to WebGPU at the `DrawBinding.draw` call. To get a clean native cut, S3 must be designed alongside S0a-wide, not as a finer-grained alternative. (Pre-#63 framing treated S3 as subordinate to "FG executor"; that's no longer accurate.)

Ship native-aware renderable builders per material family (PBR, Standard, Skybox, Shadow, …) that produce `Renderable`s whose `draw()` calls our native stack. Lite's scene orchestration is untouched.

- **S3a. Generic `DrawContext` in `Renderable.draw` / `DrawBinding.draw`.**
  Upstream: change `draw(pass: GPURenderPassEncoder | GPURenderBundleEncoder, engine)` to `draw(ctx: DrawContext, engine)`. DrawContext has both a WebGPU impl and native impl. **This is U3.**
  *Pros:* Most granular. Material teams can add backends incrementally. Directly compatible with Lite's extension philosophy. Pairs cleanly with S0a-wide via the same DrawContext type.
  *Cons:* Many builders to duplicate (PBR, Standard, Skybox, Shadow, Picking, …). Equal-scope shader authoring required per backend unless we also do §S4.
  *Upstream Lite change:* interface change + builder registration for alternate backends.

- **S3b. Native-aware material registry.**
  Today materials register via the extension pattern (PBR fragments, glTF feature modules). Add a "backend" axis: each material fragment can declare a native implementation alongside its WGSL one.
  *Pros:* Perfectly aligned with §4c′ extension pillar.
  *Cons:* Doubles the surface area for every material feature. Pixel-parity per backend is ongoing maintenance.

### S4 — Hook at the shader/WGSL layer

Lite composes WGSL strings at runtime. Intercept between `composeShader()` and `device.createShaderModule(...)` to translate WGSL → native shader language via **Tint** (or Naga).

- **S4a. Tint-as-a-library inside Babylon Native.**
  Use Tint purely as a WGSL → HLSL/MSL/SPIR-V translator, feed the output to bgfx/native. Doesn't need Dawn. Pairs with S2/S3 to remove the shader-authoring double-work.
  *Pros:* Keeps Lite's WGSL authoring pillar. No re-authoring shaders in HLSL/MSL.
  *Cons:* Tint is still a C++ dependency of non-trivial size (though much smaller than Dawn). Must track Tint/WGSL spec movement.

- **S4b. Precompile WGSL → native at build time.**
  Lite's bundle build already minifies WGSL. Add a build plugin that emits multiple variants (SPIR-V/HLSL/MSL) into the bundle; runtime picks the right one.
  *Pros:* Zero runtime cost; no shader compiler in shipping binary.
  *Cons:* Runtime shader composition (shader-composer.ts) is *dynamic* in Lite — pre-compile covers only the static subset. May require refactoring material/shader composition to be build-time-resolvable, which partly fights Lite's lazy/tree-shaking model. Likely a partial win.

### S5 — Host injection upstream in Lite 🧩

Independently of backend, change Lite's `createEngine(canvas)` so the adapter/device/context acquisition is injectable from the host environment instead of calling `navigator.gpu.requestAdapter` / `canvas.getContext("webgpu")` internally.

- *Pros:* Shrinks the DOM/WebGPU surface Babylon Native must fake. Also benefits Node/Deno/Electron/Tauri testing. Tiny, focused PR.
- *Cons:* None material.
- **Recommended regardless of backend.**

### S6 — Reframe Lite's scope (use it as authoring, not as a renderer) 📦🏎️

Use Lite's public API as a scene-authoring / asset-loading DSL; render with a completely separate native engine.

- **S6a. "Lite as glTF/PBR/env loader for Babylon Native."**
  Consume `loadGltf`, `loadEnvironment`, asset containers, material descriptors; pass the data to `NativeEngine` (bgfx) which renders.
  *Pros:* Trivial to integrate (all pure data). No WebGPU anywhere on native. Keeps Lite's WGSL-exclusivity pillar intact.
  *Cons:* Throws away every line of Lite's rendering code; pixel-parity with Lite on native is our problem end-to-end. Also doesn't solve "run Lite on native" — it uses Lite as a library.
- **S6b. Extract Lite's loaders into a separate package.**
  Upstream: split `packages/babylon-lite` into `babylon-lite-core` (WebGPU renderer) and `babylon-lite-loaders` (pure-data parsers). Babylon Native depends on only the latter.
  *Pros:* Clean separation; no pillar change; widely reusable.
  *Cons:* Only makes sense if G3 is the actual goal.

### S7 — Port Lite to C++ (no JS at all) 🏎️

- **S7a. Hand-port.** Babylon Native's smallest-possible runtime client. Loses JS authorability. Maintenance-heavy fork. Addresses G2, not G1.
- **S7b. Transpile.** Immature tooling. Rejected.

### S8 — Out-of-process WebGPU interop

Run Lite in a separate WebGPU-capable process (browser / WebView / Node+Dawn); share rendered textures into Babylon Native via native shared-handle mechanisms (DXGI / IOSurface / AHardwareBuffer).

- *Pros:* Zero WebGPU in Babylon Native itself. Useful for hybrid apps.
- *Cons:* IPC overhead, input routing, lifecycle complexity. Not "Babylon Native" in the usual sense.
- *Role:* Niche. Worth mentioning for completeness.

---

## 5. Cross-cutting: what Lite upstream changes are on the table?

If changing Lite is allowed, these upstream proposals each unlock multiple native seams. Listing so we can weigh them individually.

- **U1. Host-injection hook in `createEngine`** (§S5). No pillar impact. Always do.
- **U2′. Frame-graph executor pluggable + abstract `PassContext`.** ⚠️ **Partly obsolete in v4.** PR #63 already factors the FG as a polymorphic dispatcher over `Task` instances (no privileged "main"); orchestration is essentially trivial. The leverage that *was* phrased as "executor pluggability" is now phrased more precisely as **U9** below (encoder-as-parameter so a Task can record to a non-WebGPU encoder). Keep the spirit, drop the name.
- **U2. Genericize pre-FG `RenderingContext`** to take `RecordContext` instead of `GPUCommandEncoder`. Subsumed by the FG layer; no longer relevant.
- **U3. Genericize `Renderable.draw` / `DrawBinding.draw`** to take `DrawContext` instead of `GPURenderPassEncoder | GPURenderBundleEncoder`. **Promoted to first-class** in v4: the Renderable seam is co-equal with the Task seam (you can't replace one without the other). Required for S0a-wide and S0c.
- **U4. Add a "backend" extension axis.** Each material fragment / loader extension can register a non-WGSL variant. Biggest surface change; only needed if we ship native material implementations (e.g., to avoid WGSL translation entirely).
- **U5. Split loaders into a separate package** (§S6b). Required for G3.
- **U6. Declare a shader-composition stability surface** that Tint / Naga can consume for S4a/S4b.
- **U7. Remove WebGPU types from Lite's public API.** 🆕 v4. Today `createRenderTargetTexture` returns raw `GPUTexture`/`GPUTextureView`/`GPUSampler`; Alexis flagged this in the meeting; **DK is already pushing this on PR #63** ("wrap as a normal Texture2D-compatible Babylon Lite texture so the WebGPU handles stay internal"). No additional pillar impact — purely a cleanliness ask. **Tracked upstream by DK; we do not need to re-raise.**
- **U8. Optional `keepCPUCopy` flag on GPU buffers.** 🆕 v4. Babylon.js has it; Lite currently throws away CPU data after upload. Cedric's mesh-data-sync concern + Branimir's "behind a flag" suggestion. Useful for picking, intersection, post-load mesh edits. Cheap to add; matches existing Babylon.js pattern.
- **U9. Pass encoder as a parameter to `Task.execute(encoder)` instead of reading `engine._currentEncoder`.** 🆕 v4. Tiny API change for Lite; meaningful for native because the encoder type then swaps cleanly per backend. Replaces the "U2′ executor pluggability" phrasing.

The pillar reinterpretation is still the crux: **is Lite willing to become "WebGPU-first, backend-extensible" instead of "WebGPU-exclusive"?** If yes, S0/S2/S3/S4 open up. If no, we are on S1 (Dawn/wgpu) only.

**Immediate practical asks (do this week, regardless of final choice):**
- Confirm **U7** is in flight on PR #63 (already raised by DK). No action needed if Popov72 is already addressing it.
- Open issues for **U8** and **U9** against Lite — both small, both keep doors open.
- Defer **U3** (genericize `Renderable.draw` / `DrawBinding.draw`) until we have profiling data (§5b) and a goal ranking (§3).

---

## 5a. Custom-task authoring story (Ryan's concern) 🆕 v4

If S0a-wide is the seam, the published `Task` interface (§2) is the surface external users (re-)author against. A user-written `Task.execute()` today calls WebGPU APIs directly through `engine._currentEncoder`. That means **a custom task written for Lite-on-web doesn't automatically run on Lite-on-native**.

This is Ryan's specific concern, paraphrased: *"having that be the route an external user has to go down — write the same task five times for five backends — is not great."*

Four candidate approaches, ranked roughly by user friction:

1. **Source-only with examples (Branimir's stance).** Ship Lite + native source; users adapt their task per backend, guided by examples. Lowest engine cost, highest user cost.
2. **AssemblyScript-style subset → WASM/JS/native (Ryan's suggestion).** Constrain custom tasks to a subset of TS that compiles to WebAssembly. Then either run WASM directly or transpile to native code.
3. **AI-generated native task from a JS reference (Gary's suggestion).** Internally we use AI for the work anyway; could ship a model with the engine to do the conversion. Non-deterministic — Ryan's pushback. Possibly OK as a tool, not as the contract.
4. **Wrapper-layer / abstract encoder (related to U9).** Make `Task.execute(encoder)` work against an abstract encoder type that has both WebGPU and native impls. User-facing surface stays one. Highest engine cost, lowest user cost.

This question is **only worth answering after the goal (G1/G2/G3) is ranked**. If G1, native is just "Dawn underneath" and the question vanishes. If G2/G3 with S0a-wide, this is the design problem to settle next.

**Decision deferred until profiling + goal ranking. Captured here so it doesn't get lost.**

---

## 5b. Profiling baseline — the missing data point 🆕 v4

The single largest gap exposed by the meeting is that **nobody has actual profiling data** for where Lite spends time in a non-JIT JS environment. Multiple participants flagged this:

- **Ryan:** "we haven't done deep profiling for a little while […] is the stuff you're talking about [moving lights, etc.] actually the heavy stuff or not?"
- **Sebastien:** "I'm really wondering if the pipeline itself is currently our bottleneck. […] in an unjitted environment, [physics / collision / picking] might be more of a bottleneck than the rendering itself."
- **Gary:** scene traversal is very slow on QuickJS with thousands of nodes.

Without this data, every "should X be in native?" debate is hand-waving. **This is Phase-0 spike #1** (§8) and should run before the rest.

### Proposed methodology

- **Engine under test:** QuickJS (no JIT, smallest, slowest — worst case bound). Optionally V8/JSC for comparison.
- **Scenes:** representative real workloads, not micro-benchmarks. At minimum: BoomBox glTF, a high-mesh-count scene (1k+ nodes), a physics-heavy scene, a picking-heavy scene, a multi-pass FG scene.
- **Measure:** per-frame time broken into bands — scene traversal, animation update, frame-graph build/execute orchestration, per-task `execute()`, per-renderable `draw()`, physics step, pick raycast, GC pauses, JS↔C crossings count.
- **Output:** flame graph or stacked bar per scene. Identify the top 3–5 bands that exceed a "native makes sense" threshold.

### Why it matters for the seam decision

- If `RenderPassTask.execute()` orchestration is dominant → S0a-wide is the right seam.
- If per-renderable `DrawBinding.draw()` is dominant → S3 is co-equal (we already suspect this).
- If physics/picking/`evaluateActiveMesh` dominate → **rendering-only seams aren't enough**; Sebastien's point gains weight; S0d (custom JS↔C wire) gets stronger.
- If GC pauses or arithmetic dominate → the answer might be *engine choice*, not native bridging.

**This data drives the goal ranking and the seam choice. It should produce by far the highest-value-per-week of any Phase-0 spike.**

---

## 6. Trade-off summary

| Option | Changes Lite? | Needs native WebGPU? | Native renderer required? | Binary size | Pixel parity risk | Strategic fit |
|---|---|---|---|---|---|---|
| ❌ **S0a-narrow** (FG executor only, Tasks WebGPU) | none | yes | no | n/a | n/a | low — orchestration cost is negligible |
| 🌟 **S0a-wide** (native `RenderPassTask`) | U7 + U9 (+ U3) | no | yes (per-task) | small | medium (native renderer) | strong if FG ships and G2 |
| **S0b** FG above Dawn | U1 nice | yes (Dawn) | no | large | low | bridges web+native |
| **S0c** FG record-to-stream | U3 (+ U9) | no | yes | smallest JS-side | medium | minimal JS footprint |
| 🆕 **S0d** Custom JS↔C wire above FG | new bespoke contract | no | yes | small | medium | **scope beyond rendering** (physics, picking, traversal) |
| **S1a** Dawn | U1 nice | yes (Dawn) | no | large | low | high (also benefits full BJS WebGPU) |
| **S1b** wgpu-native | U1 nice | yes (wgpu) | no | medium | medium | medium |
| **S1e** WebView rig | none | via browser | no | n/a (rig only) | n/a | bring-up only |
| **S2** RenderingContext hook | U2 | no | yes | small | medium–high | subsumed by S0 once FG lands |
| **S3** Renderable/Builder hook | U3 (+ U4) | no | yes | small | medium–high | **co-equal with S0a-wide** in v4 |
| **S4a** Tint-only | possibly U6 | no | via S0/S2/S3 | medium (Tint) | low (WGSL preserved) | pairs with S0 |
| **S6** Lite as loader | U5 nice | no | yes (existing) | small | high (new renderer) | solves G3 only |
| **S7** C++ port | n/a | no | native | smallest | high (reimpl) | solves G2 radically |

---

## 7. Recommended direction (conditional on goals)

### Framing
This section is **directional, not decisional.** No option is pre-crowned. The right path depends on (a) which goal (G1/G2/G3) is primary, (b) whether the "WebGPU Exclusive" pillar is negotiable, and (c) **profiling data we don't yet have** (§5b). The text below names the *leading candidate* for each goal — there's usually a close second.

### Updated post-meeting (2026-04-30)
The room leaned **away from S1/Dawn-polyfill** (binary size, Lite's spirit) and **toward native execution** of more than just FG orchestration. Branimir/Sergio/Sebastien aligned (with different specifics) on "minimize JS↔native crossings; do the heavy work native." Ryan and Sebastien correctly flagged that we don't have data to confirm what *the* heavy work is. Net effect on the rankings below: **G2 candidates strengthen; G1 (Dawn) correspondingly weakens unless someone pushes it explicitly.**

### If G1 is primary ("run Lite unmodified on native"):
- **Leading candidate:** **S1a (Dawn) + S1c (webgpu.h abstraction) + S5/U1.** Lite JS runs unchanged at every layer — cleanest match for "unmodified."
- **Close alternative:** **S0a-wide (native `RenderPassTask`) + U7 + U9 + U3** — if PR #63 is the substrate, "unmodified at the authoring level" is almost as strong and binary size is much smaller. Pixel-parity risk higher.
- Use **S1e (WebView rig)** as validation harness during bring-up. Evaluate **S1d** as an accelerator. Reject **S1f**.

### If G2 is primary ("Lite's size/perf on native"):
- **Leading candidate:** **S0a-wide + S4a (Tint) + U1 + U7 + U9 (+ U3).** Smallest native binary, reuses Lite's scene/material/loader code, drives GPU through Dawn-the-library or direct backends. Each `Task` type becomes one porting unit.
- **Close alternative (new in v4):** **S0d (custom JS↔C wire)** if profiling shows non-rendering work (physics, picking, scene traversal) dominates. Wider scope than Tasks; bigger design effort.
- Either requires the "WebGPU-shape-exclusive" pillar reinterpretation (§5).

### If G3 is primary ("use Lite's loaders/descriptors on native"):
- **S6a (use as authoring API),** then **S6b/U5 (split loader package).** Cheap, independent of FG, independent of all backend questions.

### Most likely: combine
- **Profile first (§5b).** Until we know where time actually goes in non-JIT JS, the rest of this section is opinion.
- Adopt **U1** (host-injection upstream in Lite) — unambiguously good.
- **U7 is already in flight on PR #63** via DK's review. Confirm and move on.
- File issues for **U8** and **U9** against Lite this week — both cheap, both keep doors open.
- After profiling: spike **S0a-wide** (native `RenderPassTask` against PR #63's `Task` interface) and **S1a** (Dawn build) in parallel. Compare binary size, pixel parity, build complexity.
- Keep **S1e (WebView rig)** as a side harness throughout.
- **Explicitly reject:** S0a-narrow (no value), S1f (WebGPU→bgfx shim), S1g (record-and-replay fake device), S7b (TS→C++ transpile), S8 (out-of-process IPC) unless a concrete customer pushes us there.

---

## 8. Phasing (deliberately generic — re-scope per chosen goal)

**Phase 0 — Spikes (produce data, no commitments)**
- **P0-0 *(top priority, new in v4)*: Profiling baseline (§5b).** Run a representative scene (BoomBox + a heavy-mesh scene + a physics scene) on QuickJS-hosted Lite. Produce per-band timing. Drives every downstream decision.
- **P0-1: Track PR #63 to merge.** Confirm U7 (no WebGPU types in public API) is addressed. Confirm Tasks remain the canonical seam.
- **P0-2: File Lite issues for U8 (`keepCPUCopy`) and U9 (encoder-as-parameter).** Both small; both expand the option space.
- **P0-3: Build Dawn into Babylon Native; run a hello-triangle from JS via NAPI. Measure binary size per platform.**
- **P0-4: Same with wgpu-native.**
- **P0-5: Native `RenderPassTask` prototype.** Implement `createRenderPassTask` against an abstract encoder backed by Dawn-as-library, render BoomBox. Compare parity against Lite web and FG-on-Dawn (S0b). This is the S0a-wide spike.
- **P0-6: Stand up WebView2 / WKWebView host running Lite `lab/` — produces parity reference captures.**
- **P0-7: Integrate Tint standalone; translate one of Lite's PBR WGSL shaders to HLSL + MSL.**
- **P0-8: Draft upstream U1 (host-injection hook) PR against Lite.**

**Phase 1 — Pick a direction**
Based on Phase-0 data: profiling, binary size, pixel-parity measurements, the FG `Task` shape post-merge, Lite team's appetite for U7/U8/U9/U3, and which of G1/G2/G3 the org signs up for. Write a Phase-1 plan specific to that direction.

**Phase 2+ — Execution**
Defined post-Phase-1.

---

## 9. Cross-cutting risks & design notes (applies to any path)

- **WGSL compilation at runtime.** Lite composes shader strings dynamically. Any native path needs either a runtime WGSL compiler (Tint / Naga, ~MBs) or a significant refactor to static shader composition. Budget this into binary size.
- **Pixel parity.** Lite's bar is MAD against Babylon.js on WebGPU (Chrome/Dawn/Tint). Anything that isn't Dawn has measurable divergence risk (WGSL validators, fp16 policies, default MSAA, sRGB handling).
- **Canvas / present / preferred format.** `navigator.gpu.getPreferredCanvasFormat()` is browser-policy. Any native equivalent must match Lite's expectations or pixel-parity silently shifts.
- **Thread model.** Babylon Native has a dedicated render thread. WebGPU assumes a single consumer from the browser main thread. Confirm Dawn/wgpu usage crossing threads is spec-legal.
- **External texture interop.** Existing Babylon Native customers hand textures in from host D3D11/Metal. Any chosen path needs a story (`GPUExternalTexture`, Dawn's shared-handle APIs, etc.).
- **Binary size.** Dawn is the heaviest option. **See §C for concrete numbers and a Lite-specific assessment.** For iOS/Android, a build-time opt-out (`BABYLON_NATIVE_WEBGPU=ON/OFF`) is non-negotiable.
- **bgfx coexistence.** Short-to-medium term, bgfx and any new backend must coexist. Longer-term rationalization is a separate conversation.
- **Tree-shaking & side effects.** Lite's pillar forbids module-level side effects. Any host glue (especially U1) must be implementable without introducing them.
- **Loader-only consumers (G3).** Splitting loaders (U5) must preserve tree-shaking boundaries (no cross-package side effects).

---

## 10. Non-goals for this document

- Picking a ship date.
- Choosing a JS host (V8 / JSC / Chakra) — S1 is NAPI-capable-engine-agnostic.
- Replacing bgfx globally.
- Committing to specific upstream Lite changes beyond U1 (recommended regardless).

---

## 11. Decisions requested from the team

1. **Rank G1 / G2 / G3.** Which goal is primary? Which is out of scope? (Meeting consensus leaned toward G2; not formally ranked.)
2. **Confirm the "don't preclude" framing.** The goal of this document is to keep the native path viable, validated through Phase-0 prototypes (not a final implementation). Output = investigation + prototype tickets.
3. Is the **"WebGPU Exclusive" pillar** of Lite negotiable (i.e., reframe to "WebGPU-shape-exclusive; backends pluggable")? If yes, S0/S2/S3/S4 open up. If no, we are on S1.
4. Approve **U1 (host-injection hook)** upstream in Lite regardless of path — trivially useful.
5. **U7 status check:** confirm with Popov72 / DK that the "no WebGPU types in public API" rework is in flight on PR #63. (DK's review comment indicates yes.)
6. Approve filing Lite issues for **U8 (`keepCPUCopy` flag)** and **U9 (encoder-as-parameter)** this week.
7. Approve **Phase 0 spikes (§8)**, with **P0-0 (profiling baseline) prioritized first**.
8. Is **S7a (native Lite C++ port)** a separate workstream worth chartering, or out of scope?
9. First-scene target for end-to-end native render (candidate: `scene1 / BoomBox`).
10. **Custom-task authoring story (§5a):** decision deferrable; capture as an open question for whichever seam wins.

---

## A. Meeting notes — 2026-04-30

Attendees: Gary Hsu, Ryan Tremblay, Branimir Karadžić, Sebastien Vandenberghe, Alexis Vaginay, Cedric Guillemet, Sergio Zerbetto Masson. ~35 minutes.

### Stances by participant

| Participant | Position |
|---|---|
| **Branimir** | Native-first. JS only moves lights / sets booleans. Frame graph is the natural declarative seam because nothing below it should be JS. Wants pre-compiled shaders only (no Tint shipped); custom shaders allowed but not as default. |
| **Ryan** | Pragmatic. Splitting JS/native makes sense — but we don't have profiling data on what's expensive in non-JIT JS. Surfaced the custom-task-authoring problem (write-N-times for N backends). Suggested AssemblyScript-subset → WASM/native as a deterministic alternative to AI-generation. |
| **Sebastien** | Agrees with native-first direction but pushed two specific points: (a) physics / picking / collision may dominate over rendering in non-JIT JS — scope shouldn't be "rendering only"; (b) if there's a wrapper layer, AI could later rewrite a WebGPU backend on it (Office NeonBrush use case — nice-to-have, not a requirement). |
| **Alexis** | Frame-graph implementer. Two important facts: (1) FG render task in Lite is fast because it loops over machines and calls `setPipeline`/`createBindGroup`/`setBindGroup`/`draw` directly on WebGPU objects (no Babylon dispatch overhead). (2) **WebGPU types currently leak into Lite's public API and have to be removed.** Lite is WIP for months. |
| **Cedric** | Raised data-duplication: mesh in native for rendering, mesh in JS for picking → sync problem. |
| **Sergio** | Best new idea of the meeting: don't restrict the seam to FG. Design a custom JS↔C wire (shared memory buffer, etc.) sitting between Babylon-API level and FG level, optimized specifically for the boundary. → **S0d** in §4. |

### Key threads

1. **Where to put the seam.** Branimir: push native, JS scripts only. Ryan/Sebastien: profile first; the seam should follow the cost. Sergio: a custom middle-seam may beat both Babylon-level and FG-level cuts. Gary: higher-level seams reduce control & extensibility; AI being able to do everything is an open assumption.
2. **Bottleneck identification (the gap).** Sebastien: non-JIT physics/collision/picking/`evaluateActiveMesh` may dominate. Gary: scene traversal on QuickJS with thousands of nodes is very slow. **No one has actual profiling data.** → Phase-0 spike #1 (§5b).
3. **Frame graph as the seam.** Alexis described how the FG render task works in Lite (sorted opaque/transparent loops; direct WebGPU calls). Sorting and culling are separate tasks. Could only get faster with bundles (since merged in PR #63 as `_opaqueBundle` caching).
4. **Data duplication.** Cedric: render mesh vs picking mesh. Gary: hooking at FG layer keeps mesh JS-only; mesh "talks to" frame graph. Branimir: distinguish CPU-accessible (JS-modifiable) from GPU-only immutable via flag — same as Babylon.js does today. Alexis: Lite currently doesn't keep CPU copy at all; "we will have to do something." → **U8 `keepCPUCopy` flag** in §5.
5. **WebGPU types in the public API.** Alexis: "we have to be sure we don't have any WebGPU object in the API available to the user, and it's currently the case in Babylon Lite." → **U7** in §5.
6. **Shader compilation.** Branimir wants pre-compiled shaders only (don't ship Tint, ~30 MB). Gary: that restricts custom-shader scenarios (Playground, etc.). Punt: orthogonal axis from where rendering happens. Out of scope for this doc.
7. **Custom-task authoring (Ryan).** If FG is the seam, users adding tasks have to implement N times. Options: write twice, AssemblyScript-subset → WASM/native, AI-generated, source + examples. Decision deferred. → **§5a**.
8. **Sebastien's WebGPU-wrapper observation.** If we have a wrapper layer for native, AI could later rewrite a WebGPU backend on it. Not a requirement; nice-to-have for hybrid workloads.
9. **Gary's reframing.** Goal is *"don't preclude Babylon Native"*, not "implement Babylon Native for Babylon Lite." Lite team designs with native in mind. Output of this doc = investigation/prototype GitHub issues.

### Decisions reached in the meeting
- Scope is **"don't preclude," not "implement now."**
- Output is a **list of investigation/prototype GitHub issues**, not a final architecture.
- Frame graph is a key seam, but **probably not the only one**.
- WebGPU type leakage in Lite's public API must be removed.

### Still open after the meeting
- Where exactly the seam(s) live (FG vs middle-seam vs higher-level).
- Data sync strategy (native-mesh vs JS-mesh-talks-to-native).
- Custom-task authoring story.
- **Profiling baseline.** This should come first.

### Meta
The "AI-in-the-meeting" experiment (typing prompts live for the group) didn't work — too slow with 7 participants. Better workflow: each person asks their AI offline; one designated driver in the meeting; transcript fed back to AI for a post-meeting digest (which is how this document was updated to v4).

---

## B. Frame-graph PR #63 — what landed

[BabylonJS/Babylon-Lite#63](https://github.com/BabylonJS/Babylon-Lite/pull/63) — Popov72, currently open. 106 files, +2912 / −2672, 29 commits. Mergeable_state `unstable`. DK has reviewed (substantive feedback below).

### The architecture in three files

- **`frame-graph/task.ts`** — `Task` interface (42 lines): `name`, `engine`, `scene`, `record()`, `execute(): number`, `dispose()`. Encoder is read via `engine._currentEncoder` (global), not a parameter.
- **`frame-graph/frame-graph.ts`** — `FrameGraph` (88 lines): ordered `Task[]`, `build()` records each, `execute()` drains each into the encoder. Source comment: *"There is no privileged 'main' task: a scene-render task that draws into the swapchain is just one task among many."* Order is the user's responsibility (`addTask{,AtStart,Before}`).
- **`frame-graph/render-pass-task.ts`** — `RenderPassTask` (519 lines, the substance):
  - Single execute path for swapchain + offscreen.
  - Renderable population: explicit (`addToPass(mesh, opts)`) or auto-mirror from `scene._renderables` with auto-resync on `_renderableVersion`.
  - Buckets opaque / transmissive / transparent; per-frame back-to-front sort on transparent.
  - **Caches opaque draws as a `GPURenderBundle`**, invalidated by mutation/visibility version.
  - Per-pass camera override (`cam`), per-pass clear control (`clr`/`clrColor`, `"load"` mode for overlays).
  - **Each pass owns its own scene UBO + bind group 0** (fixes multi-camera/RTT correctness).
  - Calls WebGPU directly: `device.createRenderBundleEncoder`, `encoder.beginRenderPass`, `pass.setBindGroup`, `pass.executeBundles`, `pass.setViewport`, `pass.setScissorRect`, `pass.end`.

### Scene wiring

`createSceneContext` eagerly creates `scene._frameGraph` with a default `RenderPassTask` mirroring `scene._renderables` to swapchain. `scene._record()` calls `_frameGraph.execute()`. Resize triggers `_frameGraph.build()`. **All scene rendering goes through the FG.**

### What this means for native

| Observation | Implication for the plan |
|---|---|
| `Task` interface is small (4 methods) and polymorphic. | Tidy plug point. Each Task type is one porting unit. |
| FG `execute()` is a 5-line for-loop. | "Native FG executor" alone is **valueless** (S0a-narrow). Real value is in native Tasks (S0a-wide). |
| `RenderPassTask` calls WebGPU directly. | A native version is the actual target. |
| `Task.execute()` reads `engine._currentEncoder` (global). | Either swap encoder type per-engine-mode, or pass it as parameter. → **U9**. |
| Each Task iterates `binding.draw(encoder, engine)` per renderable. | The Renderable/DrawBinding seam is **co-equal** with Task; can't replace one without the other. → **U3**. |
| Opaque render-bundle caching is internal to `RenderPassTask`. | Replacing the Task gives this for free; hooking lower (S2/S3 alone) loses it. |
| Per-pass scene UBO + bind group 0. | Native task can write its own scene UBO without coordinating with neighboring tasks. Lowers data-sync surface. |
| Default scene-render is itself a Task. | Adoption-cost is zero; native swap is a Task replacement. |

### DK's review (key points)

- Parity regressions on Scene 66 / Scene 72 (PR-specific; addressed in subsequent commits).
- Bundle-size ceilings broken (asked for a bundle-reduction session before merge).
- Scene 52 parity test miswired (scene id 60, missing reference golden).
- **`createRenderTargetTexture()` exposes raw `GPUTexture` / `GPUTextureView` / `GPUSampler` — wrap before merge.** ← this is **U7** in §5; Lite team has accepted; tracked upstream.
- Standard-material extension registry leaks across the dynamic-import boundary — pre-existing but worsened by this PR.

### Public-API exports added

```ts
export type { FrameGraph } from "./frame-graph/frame-graph.js";
export { addTask, addTaskAtStart, addTaskBefore } from "./frame-graph/frame-graph-actions.js";
export type { Task } from "./frame-graph/task.js";
export type { RenderPassTask, RenderPassTaskConfig } from "./frame-graph/render-pass-task.js";
export { createRenderPassTask, removeMeshFromTask } from "./frame-graph/render-pass-task.js";
export type { RenderTarget, RenderTargetDescriptor } from "./engine/render-target.js";
export { createRenderTarget } from "./engine/render-target.js";
export { createRenderTargetTexture } from "./texture/rtt.js";  // ← U7 cleanup target
```

---

*Drafted by @bghgary with Copilot on branch `bghgary/babylon-lite-plan`. Iterate freely.*

---

## C. Dawn binary-size analysis 🆕 v4

This appendix concretizes the §9 "binary size" risk with actual measurements and assesses whether Dawn's footprint is acceptable for *Babylon-Native–hosted Lite specifically*. Lite's identity is bundle size; this question is unusually load-bearing for this plan.

### Concrete numbers

**Source 1 — Dawn official nightlies** ([github.com/google/dawn/releases](https://github.com/google/dawn/releases), v20260423.175430):

| Platform | Release tarball (compressed) | Notes |
|---|---|---|
| macOS arm64 | **9.0 MB** | single arch |
| macOS Intel | 9.8 MB | single arch |
| Apple `xcframework` | **23.2 MB** | universal: macOS arm64 + Intel + iOS arm64 + iOS sim |
| Linux x64 | 27.8 MB | includes shaderc/SPIRV-Tools artifacts |
| Windows x64 | 39.9 MB | includes PDBs and import libs |
| Android | **43.9 MB** | multi-ABI: arm64-v8a + armeabi-v7a + x86 + x86_64 |
| Headers only | 11.2 MB | source-only |

**Source 2 — Single-shared-library POC** (`webgpu-dawn-binaries` v127.0.6531, Jaswant Panchumarti / Dawn group, June 2024):

| Platform | Single shared library on disk |
|---|---|
| Windows `dawn.dll` | **8.0 MB** |
| macOS `libdawn.dylib` | **7.6 MB** |
| Linux `libdawn.so` | **15 MB** |

These are the *actual binary on disk* (not compressed tarballs). Roughly 3 MB smaller than Dawn's stock `BundleLibraries.cmake` output. All backends bundled.

**Rule of thumb:** budget **8–15 MB stripped, single-arch, single-backend** Dawn binary per platform. Multi-ABI Android fat-binaries multiply by 4×.

### Why size matters specifically for BN-Lite

Lite's [README](https://github.com/BabylonJS/Babylon-Lite) opens with: *"A WebGPU-exclusive, tree-shakable 3D engine that produces pixel-identical output to Babylon.js — **in a fraction of the bundle size**."* The whole product is built around minimizing bytes:

- Per-scene CI ceilings in `scene-config.json` are **46–104 KB raw JS** per scene.
- `pnpm test:bundle-size` is a first-class workflow.
- DK explicitly pushed back on PR #63 for breaking ceilings ("we do not give up on ceiling easily").

A native Dawn footprint is **roughly 100–300× a typical Lite scene's entire JS bundle**. That sounds catastrophic, but: **JS bundles ≠ native binaries.** Lite's bundle-size discipline applies to bytes shipped to a browser. Dawn-on-native ships in the app/installer, not over the wire to a browser. The Lite-on-web product is entirely unaffected by whatever we do for Lite-on-native.

The relevant comparison is therefore not *Lite's JS bundle* but *Babylon Native's existing native footprint*:

- Babylon Native today (bgfx + JS host + Babylon.js): community-reported native libs are roughly 7–15 MB per platform release-stripped.
- Adding Dawn to BN: **+8–15 MB per platform**, roughly doubling.
- Removing bgfx (eventual rationalization, §9): could neutralize the addition.

### Where Dawn's size is bad for BN-Lite

1. **Spirit mismatch.** Lite's brand promise is "fraction of the bundle size." Even with the JS/native distinction, shipping a 10+ MB WebGPU runtime on every native target inverts the marketing more than it inverts the engineering.
2. **Mobile sensitivity.** Some consumers are size-conscious (cellular download caps; iOS warns at 200 MB; corporate device storage limits). +10 MB per architecture compounds across multi-ABI Android builds.
3. **bgfx coexistence period.** Short-to-medium term, BN ships *both* bgfx and Dawn. That's the worst-case footprint.
4. **Branimir's "build a little small" point.** This was originally his shader-compiler argument; it generalizes: don't ship the whole world if you don't need the whole world.
5. **Tint inside Dawn.** ~2–3 MB of Dawn is the WGSL→HLSL/MSL/SPIR-V translator. Useful to have at runtime *only if* you accept dynamic shader composition (Lite does today). Pre-compiled-shader scenarios (Branimir's argument) don't need it — but Lite's current architecture does.

### Where Dawn's size is acceptable (or the right answer)

1. **Pixel parity is highest-confidence.** Lite measures MAD against Babylon.js on Chrome/Dawn/Tint. Dawn-on-native uses the same Tint and the same WGSL validator. Any non-Dawn path (wgpu-native, hand-rolled native renderer, S0a-wide) carries measurable divergence risk that has to be invested against forever.
2. **Strategic value beyond Lite.** Dawn-on-native enables full Babylon.js WebGPU on native too, not just Lite. The integration cost is shared.
3. **One-time native cost, not per-scene.** Unlike Lite's JS where every imported feature is a new KB on the wire, Dawn is a fixed link-time cost. A 1000-mesh scene pays the same Dawn footprint as a hello-triangle.
4. **Knobs exist.**
   - **Per-backend**: iOS Metal-only is much smaller than full multi-backend.
   - **No Tint at runtime**: ship pre-compiled SPIR-V/HLSL/MSL, drop ~2–3 MB. Only if Lite agrees to a precompile path (Branimir's preference; orthogonal).
   - **LTO + symbol stripping**: standard Release build can shave more.
   - **Validation off in release**: Dawn's validation layer is conditional.
5. **Fat-binary comparison is unfair.** Lite-on-Android shipping all four ABIs is uncommon; app-store thinning typically delivers a single-arch native bundle to each device. The 43.9 MB Android tarball is misleading as a "user device" number.

### Comparison: Dawn vs alternatives at the WebGPU layer

| Backend | Stripped single-arch | Pixel parity vs Lite-web | Notes |
|---|---|---|---|
| **Dawn** (S1a) | **8–15 MB** | **highest** (same Tint as Chrome) | strategic for BN beyond Lite |
| **wgpu-native** (S1b) | ~3–6 MB (community-reported) | medium (different WGSL validator) | requires Rust toolchain |
| **Native renderer** (S0a-wide) | small (write only what's needed) | medium (must be measured per material/shader) | most maintenance over time |
| **WebView host** (S1e) | n/a (not shipping) | n/a | bring-up rig only |

### Recommendation, given Lite's identity

- **For G1 (run Lite unmodified on native):** Dawn is the price of admission. The 8–15 MB hit is acceptable because (a) it's strategic for BN beyond just Lite, (b) it gives the cleanest pixel-parity story, and (c) the marketing-vs-engineering distinction is real (JS bundle ≠ native binary).
- **For G2 (Lite's size/perf identity preserved on native):** Dawn fights the brand promise. **S0a-wide (native `RenderPassTask`)** is preferable; pay the parity-engineering cost instead of the binary-size cost. wgpu-native is a middle option only if its WGSL validator's divergence proves manageable.
- **For G3 (Lite as loader):** irrelevant.
- **Either way: P0-3 (Dawn build) and P0-5 (native `RenderPassTask` prototype) need real per-platform measurements** before this paragraph stops being opinion.

### Phase-0 measurement methodology

The Phase-0 Dawn spike (P0-3, §8) should produce these numbers, not just "hello triangle works":

- **Per-platform stripped Release size** of the smallest viable Dawn build (single backend per OS: D3D12 on Windows, Metal on macOS/iOS, Vulkan on Linux/Android).
- **With and without Tint** (i.e., with and without runtime WGSL compilation).
- **Validation on / off** in Release mode.
- **bgfx + Dawn coexistence overhead** vs. either alone.
- Compare against the **S0a-wide spike** (P0-5) on the same scenes.
