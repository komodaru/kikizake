"""Build native executors from an MSVC x86 or x64 Developer Command Prompt."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
from tempfile import TemporaryDirectory

HERE = Path(__file__).resolve().parent
RUNNER = HERE.parent
CASES = RUNNER.parent / "cases"
BINDINGS = (
    "d3d8.h", "d3d8caps.h", "d3d8types.h", "dcommon.h", "d2dbasetypes.h",
    "d2d1.h", "dwrite.h", "dwrite_1.h", "dwrite_2.h", "dwrite_3.h",
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mingw-headers", type=Path, required=True, help="directory containing the MinGW-w64 C declarations")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--warp", action="store_true", help="use WARP for Direct3D 10, 10.1 and 11")
    parser.add_argument("suites", nargs="+", choices=sorted(p.stem for p in CASES.glob("*.txt")))
    args = parser.parse_args()
    target = os.environ.get("VSCMD_ARG_TGT_ARCH")
    if target not in {"x64", "x86"}:
        parser.error("run from the x64 or x86 MSVC Developer Command Prompt")
    arch = {"x64": 64, "x86": 32}[target]
    inputs = {s: (CASES / (s + ".txt")).read_bytes() for s in args.suites}
    other = b"# x86 only" if arch == 64 else b"# x64 only"
    for suite, data in inputs.items():
        if any(line.startswith(other) for line in data.splitlines()):
            parser.error(f"{suite} excludes {target}")
    sdk = next((p.parent for entry in os.environ.get("INCLUDE", "").split(";")
                if (p := Path(entry)).name.lower() == "um" and (p / "d3d12.h").is_file()), None)
    if sdk is None:
        parser.error("Windows SDK um headers missing from INCLUDE")
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    with TemporaryDirectory() as tmp:
        work = Path(tmp)
        overlay = work / "bindings"
        overlay.mkdir()
        for header in BINDINGS:
            shutil.copyfile(args.mingw_headers / header, overlay / header)
        warp = work / "warp.h"
        if args.warp:
            # Load the SDK enums before redirecting the runner's first driver choice.
            warp.write_text(
                "#define COBJMACROS\n#include <d3d10_1.h>\n#include <d3d11.h>\n"
                "#define D3D10_DRIVER_TYPE_HARDWARE D3D10_DRIVER_TYPE_WARP\n"
                "#define D3D_DRIVER_TYPE_HARDWARE D3D_DRIVER_TYPE_WARP\n",
                encoding="ascii",
            )
        flags = ["cl", "/nologo", "/std:c11", "/O2", "/c", f"/I{HERE}", f"/I{sdk}"]
        objects = []
        for source in sorted(RUNNER.glob("*.c")):
            if source.name == "main.c":
                continue
            extra = ([f"/I{overlay}", f"/FI{HERE / 'bindings.h'}"]
                     if source.stem in {"d2d", "dwrite", "d3d8draw"} else [])
            if args.warp and source.stem in {"d3d11", "d3dres"}:
                extra.append(f"/FI{warp}")
            obj = work / (source.stem + ".obj")
            subprocess.run([*flags, *extra, str(source), f"/Fo{obj}"], cwd=work, check=True)
            objects.append(str(obj))
        for suite, data in inputs.items():
            with (work / "cases.h").open("w", encoding="ascii") as header:
                header.write("static const char kikizake_cases[] = {\n")
                for i in range(0, len(data), 32):
                    header.write(",".join(map(str, data[i:i + 32])) + ",\n")
                header.write("0};\n")
            subprocess.run([*flags, f"/I{work}", '/DKIKIZAKE_CASES="cases.h"',
                            f'/DKIKIZAKE_SUITE="{suite}"', str(RUNNER / "main.c"),
                            "/Fomain.obj"], cwd=work, check=True)
            subprocess.run(["link", "/nologo", "main.obj", *objects,
                            "kernel32.lib", "gdi32.lib", "user32.lib", "ole32.lib",
                            "windowscodecs.lib", "uuid.lib", f"/out:{out / f'msvc_{suite}-{arch}.exe'}"],
                           cwd=work, check=True)


if __name__ == "__main__":
    main()
