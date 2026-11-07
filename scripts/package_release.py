"""Package a compiled native binary, documentation, and a SHA-256 checksum."""

import hashlib
from pathlib import Path
import sys
import tarfile
import tomllib
import zipfile


def main():
    target = sys.argv[1]
    allowed = {
        "x86_64-pc-windows-msvc",
        "x86_64-unknown-linux-gnu",
        "aarch64-apple-darwin",
        "x86_64-apple-darwin",
    }
    if target not in allowed:
        raise SystemExit(f"Unsupported release target: {target}")
    root = Path(__file__).resolve().parent.parent
    version = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]
    name = f"clang-tidy-gui-v{version}-{target}"
    executable = "clang-tidy-gui.exe" if "windows" in target else "clang-tidy-gui"
    files = {root / "target" / target / "release" / executable: executable}
    documents = [
        "README.md", "LICENSE", "CHANGELOG.md", "CONTRIBUTING.md",
        "doc/screenshot_main.png", "doc/screenshot_light.png",
        "examples/cpp/CMakeLists.txt", "examples/cpp/main.cpp",
    ]
    files.update({root / filename: filename for filename in documents})
    for file in files:
        if not file.is_file():
            raise SystemExit(f"Missing release input: {file}")
    destination = root / "dist"
    destination.mkdir(exist_ok=True)
    if "windows" in target:
        archive = destination / f"{name}.zip"
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as output:
            for file, relative in files.items():
                output.write(file, f"{name}/{relative}")
    else:
        archive = destination / f"{name}.tar.gz"
        with tarfile.open(archive, "w:gz") as output:
            for file, relative in files.items():
                output.add(file, arcname=f"{name}/{relative}")
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_name(archive.name + ".sha256").write_text(f"{digest}  {archive.name}\n", encoding="utf-8")
    print(archive)


if __name__ == "__main__":
    main()
