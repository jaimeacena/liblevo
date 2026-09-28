import os
from pathlib import Path

from PyInstaller.utils.hooks import (
    collect_all,
    collect_data_files,
    collect_dynamic_libs,
    copy_metadata,
)


root = Path.cwd()
notices = Path(
    os.environ.get(
        "LIBLEVO_NOTICES_PATH",
        str(root / "distribution" / "windows" / "THIRD-PARTY-NOTICES.txt"),
    )
)
if not notices.is_file():
    raise FileNotFoundError(
        "Genera THIRD-PARTY-NOTICES.txt con distribution/windows/generate_notices.py."
    )
datas = [
    (str(root / "assets" / "branding" / "generated"), "assets/branding/generated"),
    (str(root / "assets" / "fonts"), "assets/fonts"),
    (str(root / "LICENSE"), "."),
    (str(notices), "."),
]
datas.extend(collect_data_files("liblevo", includes=["licenses/*.txt"]))
binaries = []
hiddenimports = []

# The local GGUF binding loads its Vulkan libraries dynamically at runtime.
binaries.extend(collect_dynamic_libs("llama_cpp"))
datas.extend(copy_metadata("llama-cpp-python"))


def include_runtime_submodule(name):
    excluded_segments = (".tests", ".test", ".testing", ".examples", ".benchmarks")
    excluded_modules = (".__main__", ".cli")
    return not any(segment in name for segment in excluded_segments) and not (
        name.endswith(excluded_modules) or any(f"{module}." in name for module in excluded_modules)
    )


excluded_data = [
    "**/tests/**",
    "**/test/**",
    "**/testing/**",
    "**/examples/**",
    "**/benchmarks/**",
]

# Docling's public package contains CLIs, experimental VLM pipelines and format backends that this
# PDF/EasyOCR-only application never calls. Static analysis follows the actual converter imports;
# only runtime data, native libraries and the built-in model plugin need explicit collection.
for package in (
    "docling",
    "docling_core",
    "docling_ibm_models",
    "easyocr",
):
    datas.extend(collect_data_files(package, excludes=excluded_data))
    binaries.extend(collect_dynamic_libs(package))

datas.extend(copy_metadata("docling-slim"))
hiddenimports.append("docling.models.plugins.defaults")

# These two small packages discover converters/commands dynamically, so retain their runtime
# modules while still excluding development-only trees and command-line entry points.
for package in (
    "argostranslate",
    "markitdown",
):
    package_datas, package_binaries, package_hidden = collect_all(
        package,
        filter_submodules=include_runtime_submodule,
        exclude_datas=excluded_data,
    )
    datas.extend(package_datas)
    binaries.extend(package_binaries)
    hiddenimports.extend(package_hidden)

analysis = Analysis(
    [str(root / "src" / "liblevo" / "__main__.py")],
    pathex=[str(root / "src")],
    binaries=binaries,
    datas=datas,
    hiddenimports=hiddenimports,
    excludes=["spacy"],
    noarchive=False,
)

# PyTorch ships a copy of its complete third-party licence tree inside dist-info. Some paths exceed
# what Inno Setup can reopen on Windows. The generated THIRD-PARTY-NOTICES.txt above already embeds
# those texts, so omit only that redundant tree while retaining Torch's own licence and metadata.
def is_redundant_torch_license(entry):
    destination = "/" + entry[0].replace("\\", "/").casefold()
    return "/torch-" in destination and ".dist-info/licenses/third_party/" in destination


package_datas = [entry for entry in analysis.datas if not is_redundant_torch_license(entry)]


def is_foreign_poppler_icu(entry):
    # PATH may contain a separate PDF tool's ICU DLLs. Bundling them at the
    # package root shadows Qt's own dependencies and prevents QtGui importing.
    destination = entry[0].replace("\\", "/").casefold()
    source = entry[1].replace("\\", "/").casefold()
    return destination in {"icuuc.dll", "icudt78.dll"} and "/poppler/" in source


package_binaries = [entry for entry in analysis.binaries if not is_foreign_poppler_icu(entry)]
pyz = PYZ(analysis.pure)
exe = EXE(
    pyz,
    analysis.scripts,
    [],
    exclude_binaries=True,
    name="Liblevo",
    console=False,
    icon=str(root / "assets" / "branding" / "generated" / "liblevo-app-icon.ico"),
    version=str(root / "distribution" / "windows" / "version_info.txt"),
)
bundle = COLLECT(
    exe,
    package_binaries,
    package_datas,
    strip=False,
    upx=True,
    name="Liblevo",
)
