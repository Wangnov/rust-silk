import argparse
import json
import math
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tarfile
import tempfile
import zipfile


def run(command, *args, data=None):
    result = subprocess.run(
        [*command, *map(str, args)], input=data, capture_output=True, timeout=30
    )
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors="replace"))
    return result.stdout


def extract_binary(archive, destination):
    name = "rust-silk.exe" if os.name == "nt" else "rust-silk"
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as source:
            matches = [item for item in source.namelist() if Path(item).name == name]
            assert len(matches) == 1, matches
            contents = source.read(matches[0])
    else:
        with tarfile.open(archive) as source:
            matches = [item for item in source.getmembers() if item.isfile() and Path(item.name).name == name]
            assert len(matches) == 1, matches
            contents = source.extractfile(matches[0]).read()
    binary = destination / name
    binary.write_bytes(contents)
    binary.chmod(0o755)
    return binary


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser()
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--archive", type=Path)
    source.add_argument("--binary", type=Path)
    source.add_argument("--launcher", type=Path)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="rust-silk-smoke-") as directory:
        work = Path(directory)
        binary = extract_binary(args.archive, work) if args.archive else args.binary
        command = ["node", str(args.launcher.resolve())] if args.launcher else [str(binary.resolve())]
        assert b"silk-sdk" in run(command, "--version")
        for tencent in [False, True]:
            for milliseconds, packet_ms in [(1000, 20), (1000, 60), (20, 60), (5, 100), (100, 100)]:
                count = milliseconds * 24
                pcm = b"".join(
                    struct.pack("<h", int(10000 * math.sin(2 * math.pi * 440 * i / 24000)))
                    for i in range(count)
                )
                input_file = work / "input.pcm"
                input_file.write_bytes(pcm)
                silk = work / "audio.silk"
                run(command, "encode", "-i", input_file, "-o", silk,
                    "--packet-ms", packet_ms, "--quiet", *(["--tencent"] if tencent else []))
                output = work / "output.pcm"
                run(command, "decode", "-i", silk, "-o", output, "--quiet")
                samples_per_packet = packet_ms * 24
                expected_samples = (count + samples_per_packet - 1) // samples_per_packet * samples_per_packet
                decoded = output.read_bytes()
                assert len(decoded) == expected_samples * 2, (milliseconds, packet_ms, len(decoded))
                assert any(decoded), "解码结果不应全部静音"
                assert run(command, "decode", "-i", "-", "-o", "-", "--quiet", data=silk.read_bytes()) == decoded
        if not args.launcher:
            info = json.loads(subprocess.check_output([
                "node", "-p", "JSON.stringify({platform:process.platform,arch:process.arch})"
            ]))
            platform, arch = info["platform"], info["arch"]
            package = f"rust-silk-{platform}-{arch}"
            if platform == "linux":
                package += "-gnu"
            elif platform == "win32":
                package = "rust-silk-windows-x64-msvc"
            package_dir = work / "node_modules" / package
            (package_dir / "bin").mkdir(parents=True)
            (package_dir / "package.json").write_text(json.dumps({"name": package, "version": "0.0.0"}))
            shutil.copy2(binary, package_dir / "bin" / binary.name)
            launcher = work / "rust-silk.js"
            shutil.copyfile(Path(__file__).resolve().parent.parent / "npm/main/bin/rust-silk.js", launcher)
            assert run(["node", str(launcher)], "decode", "-i", "-", "-o", "-", "--quiet", data=silk.read_bytes()) == decoded
        print("发布包验收通过：编解码、尾部补齐、标准头/腾讯头和 npm 标准输入输出")


if __name__ == "__main__":
    main()
