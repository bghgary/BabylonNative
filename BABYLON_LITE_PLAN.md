# Babylon Lite on Babylon Native — Options & Plan

> **Status:** Draft for team review. Iterate in-place.
> **Audience:** Babylon Native + Babylon Lite teams.
> **Constraints:**
> - Changes to Babylon Lite are **on the table** — it is a young, single-engine codebase and we can influence its design.
> - **A frame graph for Babylon Lite is being designed in parallel** by another team member. This document **assumes the frame graph will exist**, and evaluates it as **one candidate integration target** alongside several others. It is not pre-declared as the right answer.

---

## 1. Document history

- **v1:** Framed the integration as "WebGPU polyfill (Dawn / wgpu) vs. C++ port." Too narrow.
- **v2:** Discovered Lite's existing internal seams (`RenderingContext`, `Renderable`, `MeshGroupBuilder`); added "change Lite" as a first-class axis; added the "what problem are we solving?" goal-ranking question (G1/G2/G3).
- **v3 (this version):** A frame graph is now an assumed future component of Lite. It becomes a first-class candidate integration seam alongside the v2 options — **but not automatically the winner**. Which seam is right depends on the goal (§3) and on concrete spike data (§8).

---

## 2. What Lite actually looks like inside (today + assumed frame graph)

I read the source (`D:\GitHub\BabylonJS\Babylon-Lite\packages\babylon-lite\src`, ~21k LOC of TS). Key findings about the **current** code, plus the **assumed future** frame-graph layer:

### Today: no frame graph
There is **no** FrameGraph/RenderGraph abstraction in Lite as of `master @ 3050ad4`. Rendering is a fixed pipeline with explicit stages (shadow pre-pass → opaque → transmissive → transparent), hardcoded in `scene-core.ts`.

### Tomorrow (assumed): frame graph as the orchestrator
A separate workstream is designing a frame graph for Lite. We assume it will follow the same broad shape as Babylon.js's frame graph (and Frostbite/O'Donnell-style FGs):

- **Authoring**: passes (render / compute / pre-pass) declare resources read & written as logical handles.
- **Compile**: per-frame DAG resolution → resource lifetime / aliasing, transient texture allocation, barrier insertion, render-target binding selection.
- **Execute**: a backend walks compiled passes in order and records actual GPU commands. Each pass has an `execute(context)` callback the user supplied at authoring time.

For this plan, the **assumed FG-related contract** is:
- Resources declared at the FG layer are **abstract handles** (not raw `GPUTexture` / `GPUBuffer`).
- Each pass receives a **pass-execution context** at execute time. The exact shape (raw `GPURenderPassEncoder` vs. an abstract `RecordContext`) is a design decision still in flight — and is **the single biggest leverage point for native** (see §5/U-changes).
- Existing Lite stages (shadow generator, opaque/transmissive/transparent main pass, picking) become FG passes.
- `MeshGroupBuilder`-built `Renderable`s are still consumed inside the main-pass execute callback.

### Layering once the FG lands

```
┌──────────────────────────────────────────────────────────────────────┐
│ Public API (index.ts)                                                │
│   createEngine, createSceneContext, loadGltf, createPbrMaterial, ... │
├──────────────────────────────────────────────────────────────────────┤
│ Scene / Authoring descriptors (pure data — NO GPU coupling)          │
│   Mesh, Camera, LightBase, Material, Texture2D, SceneContext,        │
│   shadow configs, ImageProcessingConfig, thin instances, asset       │
│   containers, glTF/env/HDR/KTX/Basis/Draco/.babylon loaders          │
├──────────────────────────────────────────────────────────────────────┤
│ Frame Graph  (new — assumed)                                         │
│   Pass DAG · resource handles · compile · execute                    │
│   Pass.execute(ctx) — ctx shape is the key design seam               │
├──────────────────────────────────────────────────────────────────────┤
│ MeshGroupBuilder  (per material family)                              │
│   Consumes Mesh + Material → produces Renderable[] + SceneUBO        │
│   Materials own their shaders + pipelines (core pillar 4c)           │
├──────────────────────────────────────────────────────────────────────┤
│ Renderable  (public interface)                                       │
│   draw(ctx, engine) — ctx shape currently GPURenderPassEncoder       │
├──────────────────────────────────────────────────────────────────────┤
│ WebGPU (navigator.gpu, GPUDevice, GPUBuffer, GPUTexture, WGSL, ...) │
└──────────────────────────────────────────────────────────────────────┘
```

(`RenderingContext` from v2 likely becomes an internal of the frame graph, or is replaced by it — that's a Lite-side decision.)

### Existing seams worth noting (still relevant under FG)
- **Extension pillar** (GUIDANCE.md §4c′): every optional feature must be expressed as an extension module. Friendly to adding a "backend" axis.
- **Loaders are pure data**: `loadGltf`, `loadEnvironment`, `loadHdrEnvironment`, etc. produce plain descriptors. They don't create any GPU resource until `registerScene()` runs the material builders.
- **Materials own shaders + pipelines** (pillar 4c). The renderer is generic.
- **Constraints to respect**: WebGPU-exclusive pillar (§1), tree-shakable / zero-side-effect modules (§4), pixel-perfect accuracy vs. Babylon.js (§5), frictionless portability (§6).

### What this means
Every non-GPU seam above `WebGPU` is a legitimate interception point. The integration question is *which seam do we cut at*, not "WebGPU-polyfill or C++ port."

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

---

## 4. Option space, organized by seam

Options are grouped by **where** in Lite's stack we intercept. Each option lists who owns what, upstream Lite changes required, and trade-offs. "Upstream Lite changes" ranges from *none* (seam already supports it) to *significant* (breaks or redefines a core pillar).

Legend: **🧩** = requires Lite upstream change, **🌐** = requires native WebGPU-capable surface, **🏎️** = native renderer required, **📦** = descriptor-only reuse.

### S0 — Hook at the Frame Graph executor layer (candidate)

Once Lite has an FG, one natural native boundary is **"a backend implementation of frame-graph execution"** rather than "a WebGPU implementation underneath Lite." The FG virtualizes resources (abstract handles) and pass ordering; a native executor consumes the compiled DAG and records commands against bgfx / Dawn / direct backends. Whether this is the *best* seam vs. S1/S2/S3 is an open question to resolve with data (§8).

- **S0a. Pluggable FG execution backend.** Lite's FG exposes an executor interface (`allocate transient resource`, `begin pass`, `set render target`, `draw`, `compute dispatch`, `end pass`, `submit`). Ship a **WebGPU executor** (default, on web) and a **Babylon Native executor** (on native) — the latter maps to bgfx or direct backends.
  - *Pros:* Smallest possible native-side surface — much smaller than all of WebGPU. FG already centralizes resource lifetimes + barriers, so the native executor inherits that work. Lite's scene graph, loaders, materials, shadow/picking configs all reused unmodified. No WebGPU runtime on native. Extensible in Lite's native style (S0 is just "another backend extension").
  - *Cons:* Pass-execute callbacks currently would receive a `GPURenderPassEncoder`; for S0 to work, the callback argument must be abstracted (see U2′ below) — which is a Lite upstream ask, though one already naturally raised by introducing an FG (authors would want a stable cross-backend API anyway). Pixel parity against Lite web still lands on us: same burden as any native renderer (see S4a).
  - *Lite upstream ask:* design the FG with backend pluggability and an abstract pass-execute context from day one. **This is the single highest-leverage ask and should be communicated to the FG owner this week.**

- **S0b. Frame-graph authoring on native, executor via Dawn.** Hybrid: still bring Dawn (S1a) onto native, but use the FG as the integration point. Lite's FG compiles once, Dawn executes.
  - *Pros:* Keeps S1a's guarantees (Chrome-grade Tint, Chrome-grade WebGPU) without duplicating any Lite code. The FG becomes the de-facto native-agnostic API whether we use Dawn or a direct backend.
  - *Cons:* Still pays Dawn's binary-size cost. But the FG abstracts the "host to device" contract, so swapping Dawn↔bgfx later stays cheap.

- **S0c. Record-to-stream FG executor (no JS-side GPU).** The native executor *runs in C++*; JS-side FG emits only *passes* and *resource declarations* (plain data), plus per-pass draw lists from `Renderable`s. Native C++ consumes the stream and renders.
  - *Pros:* JS never touches a `GPUDevice`; runtime cost minimal on the JS side; Lite's pillar "WebGPU Exclusive" is technically preserved because web-side remains WebGPU only.
  - *Cons:* `Renderable.draw()` still currently takes a `GPURenderPassEncoder`. Must either (i) move per-pass draw logic into the FG pass itself (i.e., materials contribute to the pass's draw stream as data, not callbacks), or (ii) upstream a `DrawContext` abstraction (see U3). Requires real collaboration with the FG author on the cross-language shape.

### Why S0 reshapes the rest of the option space

If we end up choosing S0 *and* the FG's executor API is pluggable, several v2 seams consolidate:
- **S2 (`RenderingContext`)** is largely subsumed — scene becomes an FG client, and our native integration becomes an FG executor.
- **S3 (`Renderable` / `MeshGroupBuilder`)** still matters only for the *draw callback* inside a main pass. If pass-execute is abstracted (U2′), S3's upstream asks (U3) shrink or disappear.
- **S4 (WGSL/Tint)** remains the shader-translation question — orthogonal to S0. Native FG executor still needs some way to consume shaders. Pairs naturally with S0.
- **S5/U1** (host injection) is still always-recommended and independent of S0.

If we do *not* choose S0 (e.g., we go S1a/Dawn, or S7 C++ port), the existence of an FG in Lite is largely neutral to native — it changes Lite internals but Babylon Native still intercepts at WebGPU or below.

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

### S3 — Hook at the `Renderable` / `MeshGroupBuilder` seam 🧩🏎️

Finer-grained than S2. Ship native-aware renderable builders per material family (PBR, Standard, Skybox, Shadow, …) that produce `Renderable`s whose `draw()` calls our native stack. Lite's scene orchestration is untouched.

- **S3a. Generic `DrawContext` in `Renderable.draw`.**
  Upstream: change `draw(pass: GPURenderPassEncoder | GPURenderBundleEncoder, engine)` to `draw(ctx: DrawContext, engine)`. DrawContext has both a WebGPU impl and native impl.
  *Pros:* Most granular. Material teams can add backends incrementally. Directly compatible with Lite's extension philosophy.
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

### S9 — Do nothing / wait

Keep Babylon Native on full Babylon.js. Revisit when a concrete customer needs Lite on native.
- Legitimate option if no one is blocked.

---

## 5. Cross-cutting: what Lite upstream changes are on the table?

If changing Lite is allowed, these upstream proposals each unlock multiple native seams. Listing so we can weigh them individually.

- **U1. Host-injection hook in `createEngine`** (§S5). No pillar impact. Always do.
- **U2′. Frame-graph executor is a pluggable interface, and pass-execute callbacks receive an abstract `PassContext` (not `GPURenderPassEncoder`).** (§S0). **Only required if we end up choosing S0** — but if we might, the time to influence this is *while the FG is being designed*. Retrofitting later is much more expensive. Even if we don't commit to S0, having the FG leave this door open costs Lite little.
- **U2. Genericize pre-FG `RenderingContext`** to take `RecordContext` instead of `GPUCommandEncoder`. Only relevant if we integrate before the FG lands; likely subsumed by U2′.
- **U3. Genericize `Renderable.draw`** to take `DrawContext` instead of `GPURenderPassEncoder`. Still relevant under the FG — the per-draw callback signature is independent of pass-context shape. Required for S0c and S3.
- **U4. Add a "backend" extension axis.** Each material fragment / loader extension can register a non-WGSL variant. Biggest surface change; only needed if we ship native material implementations (e.g., to avoid WGSL translation entirely).
- **U5. Split loaders into a separate package** (§S6b). Required for G3.
- **U6. Declare a shader-composition stability surface** that Tint / Naga can consume for S4a/S4b.

The pillar reinterpretation is the crux: **is Lite willing to become "WebGPU-first, backend-extensible" instead of "WebGPU-exclusive"?** This is the single biggest team decision. If yes, S0/S2/S3/S4 open up. If no, we are on S1 (Dawn/wgpu) only.

**Immediate practical ask (do this week, regardless of final choice):** engage with the FG owner to ensure the design **doesn't close the door** on S0. Pluggable executor + abstract pass-context costs Lite essentially nothing if baked in early, even if we end up not picking S0. Waiting means retrofitting later.

---

## 6. Trade-off summary

| Option | Changes Lite? | Needs native WebGPU? | Native renderer required? | Binary size | Pixel parity risk | Strategic fit |
|---|---|---|---|---|---|---|
| **S0a FG executor (native)** | U2′ | no | yes (via executor) | small | medium (native renderer) | strong if FG + G2 |
| S0b FG + Dawn executor | U2′ | yes (Dawn) | no | large | low | bridges web+native |
| S0c FG record-to-stream | U2′ (+ U3) | no | yes | smallest JS-side | medium | for minimal JS footprint |
| S1a Dawn | no (U1 nice) | yes (Dawn) | no | large | low | high (also benefits full BJS WebGPU) |
| S1b wgpu-native | no (U1 nice) | yes (wgpu) | no | medium | medium | medium |
| S1e WebView rig | no | via browser | no | n/a (rig only) | n/a | bring-up only |
| S2 RenderingContext hook | U2 | no | yes | small | medium–high | largely subsumed by S0 once FG lands |
| S3 Renderable/Builder hook | U3 (+ U4) | no | yes | small | medium–high | still useful for per-draw callbacks under S0 |
| S4a Tint-only | possibly U6 | no | via S0/S2/S3 | medium (Tint) | low (WGSL preserved) | pairs with S0 |
| S6 Lite as loader | no (U5 nice) | no | yes (existing) | small | high (new renderer) | solves G3 only |
| S7 C++ port | n/a | no | native | smallest | high (reimpl) | solves G2 radically |

---

## 7. Recommended direction (conditional on goals)

### Framing
This section is **directional, not decisional.** No option is pre-crowned. The right path depends on (a) which goal (G1/G2/G3) is primary, (b) whether the "WebGPU Exclusive" pillar is negotiable, and (c) concrete spike data (§8). The text below names the *leading candidate* for each goal — there's usually a close second.

### If G1 is primary ("run Lite unmodified on native"):
- **Leading candidate:** **S1a (Dawn) + S1c (webgpu.h abstraction) + S5/U1.** Lite JS runs unchanged at every layer — cleanest match for "unmodified."
- **Close alternative:** **S0a (native FG executor) + U2′** — if the FG lands with pluggable execution, "unmodified at the authoring level" is almost as strong and binary size is much smaller. Pixel-parity risk higher (native renderer vs. Chrome-grade Dawn).
- Use **S1e (WebView rig)** as validation harness during bring-up. Evaluate **S1d** as an accelerator. Reject **S1f**.

### If G2 is primary ("Lite's size/perf on native"):
- **Leading candidate:** **S0a (native FG executor) + S4a (Tint for WGSL) + U1 + U2′.** Smallest native binary, reuses Lite's scene/material/loader code, drives GPU through bgfx or direct backends.
- **Close alternative:** **S2 (custom `RenderingContext`) + S4a + U1 + U2** if the FG doesn't materialize or doesn't accept pluggable execution.
- Either requires the "WebGPU-shape-exclusive" pillar reinterpretation (§5).

### If G3 is primary ("use Lite's loaders/descriptors on native"):
- **S6a (use as authoring API),** then **S6b/U5 (split loader package).** Cheap, independent of FG, independent of all backend questions.

### Most likely: combine
- Adopt **U1** immediately (host-injection upstream in Lite) — unambiguously good.
- **Engage with the FG owner this week** so the design doesn't accidentally close the door on S0 (U2′). This is a *keep options open* move, not a commitment to S0.
- **Spike S0a, S1a, and S4a in parallel** in Phase 0. Produce concrete data on: binary size, pixel-parity against Lite goldens, build complexity, and Tint integration cost. **Decide between seams after the spikes, not before.**
- Keep **S1e (WebView rig)** as a side harness throughout.
- **Explicitly reject:** S1f (WebGPU to bgfx shim), S1g (record-and-replay fake device), S7b (TS to C++ transpile), S8 (out-of-process IPC) unless a concrete customer pushes us there.

---

## 8. Phasing (deliberately generic — re-scope per chosen goal)

**Phase 0 — Spikes (produce data, no commitments)**
- P0-0 *(new)*: **Engage with the FG owner.** Review the FG design, discuss U2′ (executor pluggability + abstract `PassContext`). Goal: keep the S0 option open, without necessarily committing to it. Understand where the FG sits in Lite's timeline.
- P0-1: Build Dawn into Babylon Native; run a hello-triangle from JS via NAPI. Measure binary size per platform.
- P0-2: Same with wgpu-native.
- P0-3: Once the FG executor interface exists (even as a draft), prototype an S0a native executor that renders one Lite scene (e.g., BoomBox) via bgfx. Compare parity against Lite web reference.
- P0-4: Stand up WebView2 host running the Lite `lab/` — produces parity reference captures.
- P0-5: Integrate Tint standalone; translate one of Lite's PBR WGSL shaders to HLSL + MSL.
- P0-6: Draft upstream U1 (host-injection hook) PR against Lite.

**Phase 1 — Pick a direction**
Based on spike data: binary size, pixel-parity measurements, FG executor interface as it lands, Lite team's appetite for U2′ / U3, and which of G1/G2/G3 the org signs up for. Write Phase 1 plan specific to that direction.

**Phase 2+ — Execution**
Defined post-Phase-1.

---

## 9. Cross-cutting risks & design notes (applies to any path)

- **WGSL compilation at runtime.** Lite composes shader strings dynamically. Any native path needs either a runtime WGSL compiler (Tint / Naga, ~MBs) or a significant refactor to static shader composition. Budget this into binary size.
- **Pixel parity.** Lite's bar is MAD against Babylon.js on WebGPU (Chrome/Dawn/Tint). Anything that isn't Dawn has measurable divergence risk (WGSL validators, fp16 policies, default MSAA, sRGB handling).
- **Canvas / present / preferred format.** `navigator.gpu.getPreferredCanvasFormat()` is browser-policy. Any native equivalent must match Lite's expectations or pixel-parity silently shifts.
- **Thread model.** Babylon Native has a dedicated render thread. WebGPU assumes a single consumer from the browser main thread. Confirm Dawn/wgpu usage crossing threads is spec-legal.
- **External texture interop.** Existing Babylon Native customers hand textures in from host D3D11/Metal. Any chosen path needs a story (`GPUExternalTexture`, Dawn's shared-handle APIs, etc.).
- **Binary size.** Dawn is the heaviest option. For iOS/Android, a build-time opt-out (`BABYLON_NATIVE_WEBGPU=ON/OFF`) is non-negotiable.
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

1. **Rank G1 / G2 / G3.** Which goal is primary? Which is out of scope?
2. Is the **"WebGPU Exclusive" pillar** of Lite negotiable (i.e., reframe to "WebGPU-shape-exclusive; backends pluggable")? If yes, S0/S2/S3/S4 open up. If no, we are on S1.
3. **Engage with the FG owner this week** to discuss **U2′** (executor pluggability + abstract `PassContext`). Goal is to *keep the S0 option open*, not to commit to it.
4. Approve **U1 (host-injection hook)** upstream in Lite regardless of path — trivially useful.
5. Approve **Phase 0 spikes** (P0-0 FG coordination, S0a executor prototype, S1a Dawn build, S1e WebView rig, S4a Tint spike, U1 PR) before committing to an implementation path.
6. Is **B1/S7a (native Lite C++ port)** a separate workstream worth chartering, or out of scope?
7. First-scene target for end-to-end native render (candidate: `scene1 / BoomBox`).

---

*Drafted by @bghgary with Copilot on branch `bghgary/babylon-lite-plan`. Iterate freely.*
