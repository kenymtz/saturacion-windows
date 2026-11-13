# Generates src/igcl/bindings.rs from vendor/igcl/igcl_api.h with bindgen.
#
# Must be run from the repository root:
#   ./vendor/igcl/generate-bindings.ps1
#
# Requirements:
#   - bindgen-cli 0.73.2 (cargo install bindgen-cli --version 0.73.2)
#   - LIBCLANG_PATH pointing to LLVM's bin directory (e.g. C:\Program Files\LLVM\bin)
#
# The header is pinned to Intel IGCL commit 1db55c364b15649427a71a05d950ab9c1eaf9a07.
# Regeneration with the same bindgen version must be byte-identical to the committed file.

$ErrorActionPreference = 'Stop'

$bindgenArgs = @(
    'vendor/igcl/igcl_api.h',
    '-o', 'src/igcl/bindings.rs',
    '--dynamic-loading', 'ControlLib',
    '--dynamic-link-require-all',
    '--no-prepend-enum-name',
    '--allowlist-function', 'ctlInit|ctlClose|ctlEnumerateDevices|ctlEnumerateDisplayOutputs|ctlGetDisplayProperties|ctlPixelTransformationGetConfig|ctlPixelTransformationSetConfig',
    '--allowlist-var', 'CTL_IMPL_MAJOR_VERSION|CTL_IMPL_MINOR_VERSION',
    '--allowlist-type', '_ctl_display_config_flag_t',
    '--',
    '-include', 'stdbool.h'
)

& bindgen @bindgenArgs
if ($LASTEXITCODE -ne 0) {
    throw "bindgen failed with exit code $LASTEXITCODE"
}

Write-Host 'Generated src/igcl/bindings.rs'
