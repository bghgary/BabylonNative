# Regenerates src/bindings.rs from the C99 headers. Run after any change to
# babylon_lite.h or babylon_lite_shader_compiler.h and commit the result.
# Requires bindgen-cli (cargo install bindgen-cli) and libclang (LIBCLANG_PATH).
param(
    [string]$Bindgen = "bindgen"
)

$ErrorActionPreference = "Stop"
$crate = $PSScriptRoot
$repo = Resolve-Path (Join-Path $crate "..\..\..")
$liteInclude = Join-Path $repo "Core\LiteLayer\Include"
$compilerInclude = Join-Path $repo "Core\LiteShaderCompiler\Include"
$output = Join-Path $crate "src\bindings.rs"

& $Bindgen (Join-Path $crate "wrapper.h") `
    --output $output `
    --allowlist-function "bl_.*" `
    --allowlist-type "bl_.*" `
    --allowlist-var "BL_.*" `
    --default-enum-style newtype `
    --clang-macro-fallback `
    --merge-extern-blocks `
    --use-core `
    --with-derive-default `
    -- `
    --target=x86_64-pc-windows-msvc `
    "-I$liteInclude" `
    "-I$compilerInclude"
if ($LASTEXITCODE -ne 0)
{
    throw "bindgen failed with exit code $LASTEXITCODE"
}
