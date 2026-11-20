"""Checks a published release the way a user's copy of ZhuyinType receives it (release.yml runs it after publishing):

    python3 -I tools/verify-release.py v0.7.3 [--keep DIR]

- Every asset matches the digest GitHub reports for it, and SHA256SUMS-desktop.txt.
- The stable names README links to (ZhuyinType-macOS.pkg, ...) are byte for byte the versioned files, and
  releases/latest/download/<stable name> serves them.
- When the release is the latest: desktop-update.json and appcast.xml on main (as raw.githubusercontent.com serves
  them, which is what installed copies read) name this version.  The feed's envelope and each installer it lists carry
  a valid Ed25519 signature for the key the apps hold (zt_core::update), and so do the appcast's archive and deltas
  (Sparkle's sparkle:edSignature), each with the length it states.  A tampered feed must fail, so a broken checker
  cannot pass everything.

Standard library only, plus gh (logged in or GH_TOKEN) and an OpenSSL with Ed25519 (OpenSSL 3; on macOS Homebrew's).
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

REPOSITORY = os.environ.get("GH_REPO") or os.environ.get("GITHUB_REPOSITORY") or "henry753951/ZhuyinType"
# The Sparkle key's public half: SUPublicEDKey in platforms/macos/Resources/Info.plist and the desktop apps' updater.
PUBLIC_KEY = "GM1ROYQ7SH2jz3ul84jJSZuza92/I33Wr2ebEVWZ9tU="
STABLE = {
    "ZhuyinType-macOS.pkg": "ZhuyinType-{v}.pkg",
    "ZhuyinType-Windows-x64-setup.exe": "ZhuyinType-{v}-windows-x64-setup.exe",
    "ZhuyinType-Linux-amd64.deb": "zhuyintype_{v}_amd64.deb",
    "ZhuyinType-Linux-x86_64.rpm": "zhuyintype-{v}.x86_64.rpm",
}
FEED_WAIT = 600  # raw.githubusercontent.com caches main's files for a few minutes

failures: list[str] = []


def ok(label: str) -> None:
    print(f"ok    {label}")


def fail(label: str) -> None:
    print(f"FAIL  {label}")
    failures.append(label)


def gh(*args: str) -> str:
    return subprocess.run(["gh", *args], check=True, capture_output=True, text=True).stdout


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for block in iter(lambda: file.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def openssl() -> str:
    candidates = [os.environ.get("OPENSSL", ""), "/opt/homebrew/opt/openssl@3/bin/openssl", "/usr/local/opt/openssl@3/bin/openssl"]
    candidates.append(shutil.which("openssl") or "")
    for candidate in candidates:
        if candidate and Path(candidate).exists():
            version = subprocess.run([candidate, "version"], capture_output=True, text=True).stdout
            if version.startswith("OpenSSL 3"):
                return candidate
    raise SystemExit("verify-release: needs OpenSSL 3 for Ed25519 (brew install openssl@3, or set OPENSSL)")


class Verifier:
    def __init__(self, work: Path) -> None:
        self.openssl = openssl()
        self.work = work
        der = bytes.fromhex("302a300506032b6570032100") + base64.b64decode(PUBLIC_KEY)  # SubjectPublicKeyInfo
        self.key = work / "key.pem"
        self.key.write_text(f"-----BEGIN PUBLIC KEY-----\n{base64.b64encode(der).decode()}\n-----END PUBLIC KEY-----\n")

    def valid(self, message: Path, signature: str) -> bool:
        raw = base64.b64decode(signature.strip())
        if len(raw) != 64:
            return False
        sig = self.work / "signature.bin"
        sig.write_bytes(raw)
        command = [self.openssl, "pkeyutl", "-verify", "-pubin", "-inkey", str(self.key), "-rawin"]
        result = subprocess.run([*command, "-in", str(message), "-sigfile", str(sig)], capture_output=True, text=True)
        return result.returncode == 0 and "Success" in result.stdout

    def check(self, label: str, message: Path, signature: str, length: int | None = None) -> None:
        if length is not None and message.stat().st_size != length:
            fail(f"{label}: {message.stat().st_size} bytes, the feed says {length}")
        elif self.valid(message, signature):
            ok(f"{label}: signature")
        else:
            fail(f"{label}: signature")


def fetch_main(name: str, expect: str) -> str:
    """main's NAME as raw.githubusercontent.com serves it, once it contains EXPECT (its CDN lags behind pushes)."""
    url = f"https://raw.githubusercontent.com/{REPOSITORY}/main/{name}"
    deadline = time.monotonic() + FEED_WAIT
    while True:
        try:
            with urllib.request.urlopen(f"{url}?t={int(time.time())}", timeout=30) as response:
                text = response.read().decode()
        except urllib.error.URLError as error:
            text = f"<{error}>"
        if expect in text or time.monotonic() > deadline:
            return text
        time.sleep(20)


def latest_serves(stable: str) -> bool:
    request = urllib.request.Request(f"https://github.com/{REPOSITORY}/releases/latest/download/{stable}", method="HEAD")
    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            return bool(response.status == 200)
    except urllib.error.URLError:
        return False


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("tag", help="the release's tag, e.g. v0.7.3")
    parser.add_argument("--keep", type=Path, help="download into this directory and keep it")
    args = parser.parse_args()
    tag: str = args.tag
    version = tag.removeprefix("v")
    work = Path(tempfile.mkdtemp(prefix="verify-release-"))
    assets = args.keep or work / "assets"
    assets.mkdir(parents=True, exist_ok=True)
    verifier = Verifier(work)

    release = json.loads(gh("release", "view", tag, "-R", REPOSITORY, "--json", "assets,isDraft,isPrerelease"))
    if release["isDraft"] or release["isPrerelease"]:
        fail(f"{tag} is a draft or a prerelease")
    print(f"== {tag}: {len(release['assets'])} assets")
    for asset in release["assets"]:
        path = assets / asset["name"]
        digest = (asset.get("digest") or "").removeprefix("sha256:")
        # A file kept from an earlier run (--keep) is downloaded again only when it is not the published one.
        if not (path.exists() and path.stat().st_size == asset["size"] and (not digest or sha256(path) == digest)):
            gh("release", "download", tag, "-R", REPOSITORY, "--dir", str(assets), "--clobber", "--pattern", asset["name"])
        if not path.exists():
            fail(f"{asset['name']}: not downloaded")
        elif digest and sha256(path) != digest:
            fail(f"{asset['name']}: differs from the digest GitHub reports")
        elif path.stat().st_size != asset["size"]:
            fail(f"{asset['name']}: {path.stat().st_size} bytes, GitHub says {asset['size']}")
        else:
            ok(f"{asset['name']}: {asset['size']} bytes{', digest' if digest else ''}")

    sums = assets / "SHA256SUMS-desktop.txt"
    if sums.exists():
        for line in sums.read_text().splitlines():
            expected, name = line.split(maxsplit=1)
            path = assets / name.lstrip("*")
            if path.exists() and sha256(path) == expected:
                ok(f"SHA256SUMS-desktop.txt: {path.name}")
            else:
                fail(f"SHA256SUMS-desktop.txt: {path.name}")
    else:
        fail("SHA256SUMS-desktop.txt is missing")

    latest = gh("release", "view", "-R", REPOSITORY, "--json", "tagName", "--jq", ".tagName").strip()
    for stable, pattern in STABLE.items():
        versioned = assets / pattern.format(v=version)
        copy = assets / stable
        if not (versioned.exists() and copy.exists()):
            fail(f"{stable} or {versioned.name} is missing")
        elif sha256(versioned) != sha256(copy):
            fail(f"{stable} differs from {versioned.name}")
        else:
            ok(f"{stable} is {versioned.name}")
        if latest == tag:
            (ok if latest_serves(stable) else fail)(f"releases/latest/download/{stable}")

    if latest != tag:
        print(f"== {tag} is not the latest release ({latest}); main's feeds are not checked")
    else:
        print("== feeds on main")
        envelope_text = fetch_main("desktop-update.json", f'\\"version\\":\\"{version}\\"')
        try:
            envelope = json.loads(envelope_text)
            feed = json.loads(envelope["feed"])
        except (ValueError, KeyError):
            fail("desktop-update.json is not a signed feed")
        else:
            message = work / "feed.bin"
            message.write_bytes(envelope["feed"].encode())
            tampered = work / "tampered.bin"
            tampered.write_bytes(envelope["feed"].encode() + b" ")
            if verifier.valid(tampered, envelope["signature"]):
                fail("the checker accepted a tampered feed")
            (ok if feed.get("version") == version else fail)(f"desktop-update.json names {feed.get('version')}")
            verifier.check("desktop-update.json envelope", message, envelope["signature"])
            for name, entry in feed.get("assets", {}).items():
                path = assets / entry["url"].rsplit("/", 1)[1]
                if not entry["url"].startswith(f"https://github.com/{REPOSITORY}/releases/download/{tag}/"):
                    fail(f"{name}: {entry['url']} is not in {tag}")
                elif not path.exists():
                    fail(f"{name}: {path.name} is not in the release")
                else:
                    verifier.check(f"{name} {path.name}", path, entry["signature"], entry["length"])

        appcast = fetch_main("appcast.xml", f"<sparkle:shortVersionString>{version}<")
        items = appcast.split("<item>")[1:]
        shown = re.search(r"<sparkle:shortVersionString>(.*?)<", items[0]) if items else None
        (ok if shown and shown.group(1) == version else fail)(f"appcast.xml names {shown and shown.group(1)}")
        for tag_text in re.findall(r"<enclosure\b[^>]*>", items[0] if items else ""):
            attributes = dict(re.findall(r'([\w:]+)="([^"]*)"', tag_text))
            path = assets / attributes.get("url", "").rsplit("/", 1)[-1]
            signature = attributes.get("sparkle:edSignature")
            if not signature or not path.exists():
                fail(f"appcast {path.name}: no signature or not in the release")
            else:
                verifier.check(f"appcast {path.name}", path, signature, int(attributes.get("length", "-1")))

    if not args.keep:
        shutil.rmtree(work, ignore_errors=True)
    if failures:
        print(f"\n{len(failures)} check(s) failed")
        return 1
    print(f"\n{tag}: every check passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
