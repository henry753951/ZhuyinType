"""The engine's reading channel in Python: the variants and costs `engine/crates/zhuyin-core/src/reading_channel.rs`
gives every lexicon row, from the same channel.tsv and taiwan.tsv (resources/formats/reading-channel.md).

    python -m zhuyintype_ml.training.lexicon.engine_channel --channel DIR --lexicon lexicon.tsv [--cli zhuyin-cli]
        [--option ...]

compares this table with `zhuyin-cli --channel-table` row by row (the Python↔Rust consistency check; the unit test
runs it on a small table).  Python builds the lexicon, misread-dev and v3's reading noise from the same table, so the
two must agree on which readings reach which word and at what cost.

For a word read `syllable` at a position holding `char` (a single-character word only through its best reading):

  taiwan-word   (longer words) taiwan.tsv's Taiwan reading: 0 when it is the standard, else -log strength
  neutral       (longer words, neutral syllable) another reading of the character with another syllable:
                -log neutral_full_tone - log P(r | the character's non-neutral readings with another syllable)
  channel.tsv   single character: every other reading at max(TIE, -log P(r) / P(syllable));
                unfamiliar character in a longer word: readings with another syllable at
                -log m - log (P(r) / (1 - P(syllable)))

The first class to give a typed reading keeps it; classes that are off are skipped; each class's offset is added and
variants above max_cost dropped; a word that has its own lexicon row for the variant reading keeps the row.
Third-tone sandhi needs the next typed syllable and is not in the table.
"""

from __future__ import annotations

import argparse
import math
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

from .channel import MECHANISMS, Channel, load

CLASSES = (*MECHANISMS, "taiwan-word", "neutral", "sandhi")
TIE = 0.01
TONES = "ˊˇˋ˙"

Row = tuple[str, str, int, str, str, str, float]  # reading, word, position, typed, class, state, cost


@dataclass
class Settings:
    classes: set[str] = field(default_factory=lambda: set(CLASSES))
    corrections: set[str] = field(default_factory=lambda: {"series"})
    single_classes: set[str] = field(default_factory=lambda: set(CLASSES))
    offsets: dict[str, float] = field(default_factory=dict)
    max_cost: float = 8.0

    def arguments(self) -> list[str]:
        """The same settings as zhuyin-cli options."""
        args = ["--channel-classes", ",".join(c for c in CLASSES if c in self.classes)]
        args += ["--channel-single-classes", ",".join(c for c in CLASSES if c in self.single_classes) or "none"]
        for name, value in sorted(self.offsets.items()):
            args += ["--channel-offset", f"{name}={value!r}"]
        for name in CLASSES:
            state = "correction" if name in self.corrections else "accepted"
            args += ["--channel-state", f"{name}={state}"]
        return [*args, "--channel-max-cost", repr(self.max_cost)]


def is_han(char: str) -> bool:
    c = ord(char)
    return 0x3400 <= c <= 0x9FFF or 0xF900 <= c <= 0xFAFF or 0x20000 <= c <= 0x3134F


def base(syllable: str) -> str:
    return "".join(c for c in syllable if c not in TONES)


def variants(
    channel: Channel, char: str, syllable: str, single: bool, settings: Settings
) -> list[tuple[str, str, float]]:
    """(typed, class, cost) for `char` read `syllable` in a word of one character (`single`) or more."""
    found: dict[str, tuple[str, float]] = {}

    def add(typed: str, kind: str, cost: float) -> None:
        if kind in settings.classes and typed not in found:
            found[typed] = (kind, cost)

    shares = channel.share.get(char, {})
    if not single and (char, syllable) in channel.taiwan:
        taiwan, strength = channel.taiwan[(char, syllable)]
        add(taiwan, "taiwan-word", 0.0 if (char, syllable) in channel.standard else -math.log(strength))
    if syllable.endswith("˙"):
        if not single:
            others = {r: p for r, p in shares.items() if not r.endswith("˙") and base(r) != base(syllable)}
            total = 0.0
            for p in others.values():  # in order, as Rust sums (Python's sum() compensates since 3.12)
                total += p
            for r, p in others.items():
                add(r, "neutral", -math.log(channel.neutral) - math.log(p / total))
    elif single:
        top = shares.get(syllable)
        if top:
            for r, p in shares.items():
                if r != syllable and channel.mechanism[(char, r)] in settings.single_classes:
                    add(r, channel.mechanism[(char, r)], max(TIE, -math.log(p / top)))
    elif char not in channel.familiar:
        rest = 1.0 - shares.get(syllable, 0.0)
        if rest > 0:
            for r, p in shares.items():
                if r != syllable and p > 0 and base(r) != base(syllable):
                    add(r, channel.mechanism[(char, r)], -math.log(channel.misread) - math.log(p / rest))
    result = []
    for typed, (kind, cost) in found.items():
        cost += settings.offsets.get(kind, 0.0)
        if cost <= settings.max_cost:
            result.append((typed, kind, cost))
    return result


def read_lexicon(path: Path) -> list[tuple[str, str, float]]:
    """lexicon.tsv's rows as the engine accepts them: three fields, a reading and a word, a finite score."""
    rows = []
    for line in path.read_text(encoding="utf-8").split("\n"):
        if line.startswith("#"):
            continue
        parts = line.rstrip("\r").split("\t")
        if len(parts) != 3 or not parts[0] or not parts[1]:
            continue
        try:
            score = float(parts[2])
        except ValueError:
            continue
        if math.isfinite(score):
            rows.append((parts[0], parts[1], score))
    return rows


def table(channel: Channel, lexicon: list[tuple[str, str, float]], settings: Settings) -> list[Row]:
    """Every variant of every aligned lexicon row, sorted as zhuyin-cli --channel-table prints them."""
    readings: dict[str, list[str]] = {}
    best: dict[str, tuple[str, float]] = {}
    for reading, word, score in lexicon:
        readings.setdefault(word, []).append(reading)
        if word not in best or best[word][1] < score:
            best[word] = (reading, score)
    result: list[Row] = []
    seen: set[tuple[str, str]] = set()
    for reading, word, _ in lexicon:
        if (reading, word) in seen:
            continue
        seen.add((reading, word))
        syllables = [s for s in reading.split(" ") if s]
        if len(word) != len(syllables) or not all(is_han(c) for c in word):
            continue
        single = len(word) == 1
        if single and best[word][0] != reading:
            continue
        others = [[s for s in r.split(" ") if s] for r in readings[word]]
        for position, (char, syllable) in enumerate(zip(word, syllables)):
            found = variants(channel, char, syllable, single, settings)
            excluded = {
                o[position]
                for o in others
                if len(o) == len(syllables)
                and o[position] != syllable
                and all(i == position or a == b for i, (a, b) in enumerate(zip(o, syllables)))
            }
            for typed, kind, cost in found:
                if typed not in excluded:
                    state = "correction" if kind in settings.corrections else "accepted"
                    result.append((reading, word, position, typed, kind, state, cost))
    result.sort(key=lambda r: (r[0].encode(), r[1].encode(), r[2], r[3].encode()))
    return result


def rust_table(cli: Path, lexicon: Path, binary: Path, settings: Settings) -> list[Row]:
    """`zhuyin-cli --channel-table` parsed."""
    output = subprocess.run(
        [str(cli), "--lexicon", str(lexicon), "--channel", str(binary), *settings.arguments(), "--channel-table"],
        check=True,
        capture_output=True,
        text=True,
        encoding="utf-8",
    ).stdout
    rows = []
    for line in output.splitlines():
        reading, word, position, typed, kind, state, cost = line.split("\t")
        rows.append((reading, word, int(position), typed, kind, state, float(cost)))
    return rows


def compare(python: list[Row], rust: list[Row]) -> list[str]:
    """Differences between the two tables (empty when they agree exactly, costs bit for bit)."""
    problems = []
    if len(python) != len(rust):
        problems.append(f"{len(python)} rows in Python, {len(rust)} in Rust")
    for a, b in zip(python, rust):
        if a != b:
            problems.append(f"Python {a} / Rust {b}")
            if len(problems) > 20:
                break
    return problems


def main(argv: list[str] | None = None) -> int:
    from zhuyintype_ml.core import paths
    from zhuyintype_ml.training.lm.compile_data import compile_channel

    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--channel", type=Path, required=True, help="channel.tsv's directory")
    parser.add_argument("--lexicon", type=Path, default=paths.data("lexicon.tsv"))
    parser.add_argument("--binary", type=Path, help="channel.bin (default: compiled into a temporary directory)")
    parser.add_argument("--cli", type=Path, help="zhuyin-cli (default: built from the current sources)")
    args = parser.parse_args(argv)
    import tempfile

    settings = Settings()
    with tempfile.TemporaryDirectory() as tmp:
        binary = args.binary
        if binary is None:
            compile_channel(args.channel, Path(tmp))
            binary = Path(tmp) / "channel.bin"
        rust = rust_table(args.cli or paths.cli(), args.lexicon, binary, settings)
    python = table(load(args.channel), read_lexicon(args.lexicon), settings)
    problems = compare(python, rust)
    print(f"{len(python)} variants in Python, {len(rust)} in Rust: {'identical' if not problems else 'different'}")
    for problem in problems:
        print(problem, file=sys.stderr)
    return 1 if problems else 0


if __name__ == "__main__":
    raise SystemExit(main())
