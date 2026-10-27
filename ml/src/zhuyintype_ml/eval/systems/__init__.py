"""Input methods compared on the same keys: ZhuyinType and the other open input methods, behind one interface.

    from zhuyintype_ml.eval import systems
    system = systems.create("shanjie-chat", workers=8)

Each adapter drives the real program unmodified through the interface it documents (zhuyin-cli's batch mode, 0c's
socket protocol, 善解's CLI and C interface).  `zt systems setup` gets the others; `zt bench compare` runs them all.
"""

from __future__ import annotations

from pathlib import Path

from zhuyintype_ml.eval.systems.base import System

# Record name → (family, setup), in page order.
NAMES = (
    "zhuyintype-app",
    "zhuyintype-engine",
    "0c-statistical",
    "0c-full",
    "shanjie-chat",
    "shanjie-formal",
)


CLASSES = {
    "zhuyintype": "zhuyintype_ml.eval.systems.zhuyintype:ZhuyinType",
    "0c": "zhuyintype_ml.eval.systems.zeroc:ZeroC",
    "shanjie": "zhuyintype_ml.eval.systems.shanjie:Shanjie",
}


def capabilities(name: str) -> frozenset[str]:
    """What the system called `name` declares it is designed to do, without starting it."""
    import importlib

    family = "zhuyintype" if name.startswith("zhuyintype-") else "0c" if name.startswith("0c-") else "shanjie"
    module, cls = CLASSES[family].split(":")
    found: frozenset[str] = getattr(importlib.import_module(module), cls).capabilities
    return found


def create(
    name: str, workers: int = 8, jobs: int | None = None, data: Path | None = None, first_cpu: int | None = None
) -> System:
    """The system called `name`.  `jobs`: ZhuyinType's engine processes; `workers`: the other systems' parallel
    servers, engines or CLI processes."""
    if name.startswith("zhuyintype-"):
        from zhuyintype_ml.eval.systems.zhuyintype import ZhuyinType

        return ZhuyinType(name.removeprefix("zhuyintype-"), data, jobs)
    if name in ("0c-statistical", "0c-full"):
        from zhuyintype_ml.eval.systems.zeroc import ZeroC

        return ZeroC("statistical" if name == "0c-statistical" else "neural", workers, first_cpu)
    if name in ("shanjie-chat", "shanjie-formal"):
        from zhuyintype_ml.eval.systems.shanjie import Shanjie

        return Shanjie(name.removeprefix("shanjie-"), workers)
    raise SystemExit(f"zt: unknown system {name!r}; known: {', '.join(NAMES)}")
