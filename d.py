#!/usr/bin/env python3

import argparse
import os
import shutil
import stat
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent

DATA_SOURCES = [
    {
        "name": "stm32-data-generated",
        "repo": "https://github.com/embassy-rs/stm32-data-generated.git",
        "rev": "ede7414a7e673ece368bd697ff72de72284985d9",
    },
    {
        "name": "probe-rs",
        "repo": "https://github.com/probe-rs/probe-rs.git",
        "rev": "2e72e1c0bffa78153994f921abc728d864b8201e",
    },
]


def _rmtree(path: Path) -> None:
    "This only exists because .git dirs are read-only on Windows and shutil.rmtree won't handle that for us"

    def handle_readonly(func, p, exc_info):
        # set file/dir as writable and try the file operation again
        if not os.access(p, os.W_OK):
            os.chmod(p, stat.S_IWRITE)
            func(p)
        else:
            raise exc_info[1].with_traceback(exc_info[2])

    # you have to love python deprecating an argument on a function in stdlib.
    # the workarounds suck. change onerror to onexc when it gets removed.
    shutil.rmtree(path, onerror=handle_readonly)


def download_all() -> None:
    sources = ROOT / "sources"
    if sources.exists():
        _rmtree(sources)

    for source in DATA_SOURCES:
        name = source["name"]
        dest = sources / name
        print(f"Downloading {name}")
        subprocess.run(["git", "clone", source["repo"], str(dest), "-q"], check=True)
        subprocess.run(["git", "checkout", source["rev"]], cwd=dest, check=True)


def gen() -> None:
    shutil.rmtree(ROOT / "output", ignore_errors=True)
    subprocess.run(["cargo", "run", "--locked", "--release"], cwd=ROOT, check=True)


def main() -> None:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="subcommand", required=True)
    sub.add_parser("download-all", help="clone pinned git sources into sources/")
    sub.add_parser("gen", help="run the generator")

    match parser.parse_args().subcommand:
        case "download-all":
            download_all()
        case "gen":
            gen()


if __name__ == "__main__":
    main()
