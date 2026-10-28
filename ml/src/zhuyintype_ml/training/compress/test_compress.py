"""Unit tests of the weight quantizers (PyTorch and NumPy; skipped without them).

python -m unittest zhuyintype_ml.training.compress.test_compress
"""

from __future__ import annotations

import importlib.util
import unittest

from zhuyintype_ml.training.compress.methods import METHODS, method

HAS_TORCH = importlib.util.find_spec("torch") is not None and importlib.util.find_spec("numpy") is not None


def coremltools_linear(weight, bits, block):
    """coremltools 9.0 optimize/_utils.py quantize_weight_by_dtype (LINEAR_SYMMETRIC, signed) on fp16, then the
    dequantized fp16 values, written with NumPy as there."""
    import numpy as np

    w = weight.astype(np.float16)
    rows, columns = w.shape
    blocks = w.reshape(rows, columns // block, block) if block else w.reshape(rows, 1, columns)
    q_min, q_max = -(2 ** (bits - 1)) + 1, 2 ** (bits - 1) - 1
    val_min = np.amin(blocks, axis=-1, keepdims=True)
    val_max = np.amax(blocks, axis=-1, keepdims=True)
    max_abs = np.maximum(np.abs(val_min), np.abs(val_max))
    scale = (max_abs - -max_abs) / (q_max - q_min)
    q = np.clip(np.round(blocks / scale), q_min, q_max)
    return (q.astype(np.float32) * scale.astype(np.float32)).astype(np.float16).reshape(rows, columns)


@unittest.skipUnless(HAS_TORCH, "needs PyTorch and NumPy")
class Quantizers(unittest.TestCase):
    def setUp(self) -> None:
        import torch

        generator = torch.Generator().manual_seed(3)
        self.weight = torch.randn(64, 128, generator=generator) * 0.05
        self.weight[3, 7] = 0.9  # an outlier

    def test_linear_matches_coremltools(self) -> None:
        import numpy as np

        from zhuyintype_ml.training.compress.quantize import linear

        for bits, block in [(8, 0), (8, 32), (4, 32)]:
            got = linear(self.weight, bits, block).numpy().astype(np.float16)
            expected = coremltools_linear(self.weight.numpy(), bits, block)
            np.testing.assert_array_equal(got, expected)

    def test_linear_is_idempotent(self) -> None:
        """Core ML recomputes the scales from the fake-quantized values: they must give the same values again."""
        import torch

        from zhuyintype_ml.training.compress.quantize import linear

        for bits, block in [(8, 0), (8, 32), (4, 32)]:
            once = linear(self.weight, bits, block)
            torch.testing.assert_close(linear(once, bits, block), once, rtol=0, atol=0)

    def test_palette_tables(self) -> None:
        import torch

        from zhuyintype_ml.training.compress.quantize import fp16, palette

        for bits, group in [(4, 0), (6, 0), (4, 16)]:
            result = palette(self.weight, bits, group)
            self.assertTrue(torch.equal(result, fp16(result)), "palette values are fp16 numbers")
            groups = result.reshape(-1, (group or 64) * 128)
            self.assertTrue(all(g.unique().numel() <= 2**bits for g in groups))
            error = (result - self.weight).abs().mean() / self.weight.abs().mean()
            self.assertLess(error, 0.2 if bits == 4 else 0.05)

    def test_palette_uses_columns_when_rows_do_not_divide(self) -> None:
        from zhuyintype_ml.training.compress.quantize import palette, palette_axis

        self.assertEqual(palette_axis((419, 512), 16), 1)
        self.assertEqual(palette_axis((512, 2048), 16), 0)
        self.assertIsNone(palette_axis((419, 3), 16))
        result = palette(self.weight[:50], 4, 16)
        for column in range(0, 128, 16):
            self.assertLessEqual(result[:, column : column + 16].unique().numel(), 16)

    def test_kmeans_finds_clusters(self) -> None:
        import torch

        from zhuyintype_ml.training.compress.quantize import kmeans

        values = torch.cat([torch.full((100,), -1.0), torch.full((50,), 2.0)]).double()[None]
        self.assertEqual(kmeans(values, 2).tolist(), [[-1.0, 2.0]])

    def test_compressible_skips_small_and_bias(self) -> None:
        import torch

        from zhuyintype_ml.training.compress.quantize import compressible

        self.assertTrue(compressible("blocks.0.up.weight", torch.zeros(2048, 512)))
        self.assertFalse(compressible("tone.weight", torch.zeros(6, 512)))
        self.assertFalse(compressible("bias.weight", torch.zeros(22791, 1)))
        self.assertFalse(compressible("norm.weight", torch.zeros(512)))

    def test_stored_bytes(self) -> None:
        from zhuyintype_ml.training.compress.quantize import stored_bytes

        self.assertEqual(stored_bytes((512, 512), method("int8-channel")), 512 * 512 + 2 * 512)
        self.assertEqual(stored_bytes((512, 512), method("pal4")), 512 * 512 // 2 + 2 * 16)
        self.assertEqual(stored_bytes((512, 512), method("fp16")), 2 * 512 * 512)


class Methods(unittest.TestCase):
    def test_names_and_targets(self) -> None:
        self.assertEqual(len({m.name for m in METHODS}), len(METHODS))
        for m in METHODS:
            self.assertEqual(m.macos == "15.0", m.granularity in ("per_block", "per_grouped_channel"), m.name)


if __name__ == "__main__":
    unittest.main()
