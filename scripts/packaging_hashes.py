#!/usr/bin/env python3
"""Fill SHA-256 hashes into the Homebrew formula and WinGet installer manifest.

The manifests shipped in packaging/ are TEMPLATES: their sha256 / InstallerSha256
fields are zeroed placeholders because the release artifacts do not exist at
commit time. Run this script at release time, pointing it at a directory that
contains the published release assets, to produce submittable manifests.

Usage:
    python scripts/packaging_hashes.py --repo mail2rust --version 3.5.0 --assets-dir ./dist

It rewrites:
    packaging/homebrew/<repo>.rb
    packaging/winget/manifests/a/adpaolucci/<repo>/<version>/adpaolucci.<repo>.installer.yaml

Exit status is non-zero if any expected asset is missing.
"""
import argparse
import hashlib
import os
import re
import sys

ASSETS = {
    "mail2rust": {
        "homebrew": [
            ("aarch64-apple-darwin", "mail2rust-aarch64-apple-darwin.tar.gz"),
            ("x86_64-apple-darwin", "mail2rust-x86_64-apple-darwin.tar.gz"),
            ("x86_64-unknown-linux-gnu", "mail2rust-x86_64-unknown-linux-gnu.tar.gz"),
        ],
        "winget": "mail2rust-x86_64-pc-windows-msvc.zip",
    },
    "chrome2rust": {
        "homebrew": [
            ("aarch64-apple-darwin", "chrome2rust-aarch64-apple-darwin.tar.gz"),
            ("x86_64-apple-darwin", "chrome2rust-x86_64-apple-darwin.tar.gz"),
            ("x86_64-unknown-linux-gnu", "chrome2rust-x86_64-unknown-linux-gnu.tar.gz"),
        ],
        "winget": "chrome2rust-x86_64-pc-windows-msvc.zip",
    },
    "robocopy2rust": {
        "homebrew": [
            ("aarch64-apple-darwin", "robocopy2rust-aarch64-apple-darwin.tar.gz"),
            ("x86_64-apple-darwin", "robocopy2rust-x86_64-apple-darwin.tar.gz"),
            ("x86_64-unknown-linux-gnu", "robocopy2rust-x86_64-unknown-linux-gnu.tar.gz"),
        ],
        "winget": "robocopy2rust-x86_64-pc-windows-msvc.zip",
    },
    "android2rust": {
        "homebrew": [
            ("aarch64-apple-darwin", "android2rust-aarch64-apple-darwin.tar.gz"),
            ("x86_64-apple-darwin", "android2rust-x86_64-apple-darwin.tar.gz"),
            ("x86_64-unknown-linux-gnu", "android2rust-x86_64-unknown-linux-gnu.tar.gz"),
        ],
        "winget": "android2rust-x86_64-pc-windows-msvc.zip",
    },
}


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", required=True, choices=sorted(ASSETS))
    ap.add_argument("--version", required=True)
    ap.add_argument("--assets-dir", required=True)
    args = ap.parse_args()

    assets_dir = args.assets_dir
    repo = args.repo
    version = args.version
    cfg = ASSETS[repo]

    # Verify assets exist
    missing = []
    for _, name in cfg["homebrew"]:
        if not os.path.isfile(os.path.join(assets_dir, name)):
            missing.append(name)
    if not os.path.isfile(os.path.join(assets_dir, cfg["winget"])):
        missing.append(cfg["winget"])
    if missing:
        print("Missing release assets:", file=sys.stderr)
        for m in missing:
            print("  - " + m, file=sys.stderr)
        return 1

    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

    # Homebrew formula
    formula = os.path.join(root, "packaging", "homebrew", repo + ".rb")
    if os.path.isfile(formula):
        with open(formula, encoding="utf-8") as f:
            text = f.read()
        for _, name in cfg["homebrew"]:
            asset_path = os.path.join(assets_dir, name)
            digest = sha256_file(asset_path)
            # Replace the sha256 line that follows the matching url line.
            pattern = re.compile(
                r'(url "https://github.com/adpaolucci/'
                + re.escape(repo)
                + r'/releases/download/[^"]+/'
                + re.escape(name)
                + r'"\n\s*sha256 ")[0-9a-f]{64}(")',
                re.MULTILINE,
            )
            text, n = pattern.subn(r"\g<1>" + digest + r"\g<2>", text)
            if n == 0:
                # Fall back to replacing any 64-zero placeholder paired with this url's arch block
                text = text.replace('sha256 "0' + "0" * 63 + '"', 'sha256 "' + digest + '"', 1)
        with open(formula, "w", encoding="utf-8") as f:
            f.write(text)
        print("Updated", formula)

    # WinGet installer manifest
    winget_base = os.path.join(root, "packaging", "winget", "manifests", "a", "adpaolucci", repo)
    installer = os.path.join(winget_base, version, f"adpaolucci.{repo}.installer.yaml")
    if not os.path.isfile(installer) and os.path.isdir(winget_base):
        candidates = [
            os.path.join(winget_base, entry, f"adpaolucci.{repo}.installer.yaml")
            for entry in sorted(os.listdir(winget_base))
            if os.path.isfile(os.path.join(winget_base, entry, f"adpaolucci.{repo}.installer.yaml"))
        ]
        if len(candidates) == 1:
            installer = candidates[0]
    if os.path.isfile(installer):
        with open(installer, encoding="utf-8") as f:
            text = f.read()
        digest = sha256_file(os.path.join(assets_dir, cfg["winget"]))
        text = re.sub(
            r"(InstallerSha256:\s*)[0-9a-f]{64}",
            r"\g<1>" + digest,
            text,
        )
        with open(installer, "w", encoding="utf-8") as f:
            f.write(text)
        print("Updated", installer)

    print("Done. Review the diff before submitting.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
