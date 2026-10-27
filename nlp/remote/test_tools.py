"""Small regression checks for batch partitioning and failure propagation."""
import subprocess
import sys
import threading
import unittest

from evaluate import parallel_run

class RunnerTests(unittest.TestCase):
    def test_order_blank_lines_and_partitions(self):
        text = 'one\n\nthree\nfour\nfive\n'
        result = parallel_run(subprocess.run, [sys.executable, '-c', 'import sys; sys.stdout.write(sys.stdin.read())'],
                              4, threading.Semaphore(4), input=text, capture_output=True, text=True, check=True)
        self.assertEqual(result.stdout, text)

    def test_cli_override_reaches_evaluator(self):
        # --cli must not be consumed as an abbreviation of the adapter's --cli-arg.
        from pathlib import Path
        import tempfile
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            cli = root / 'fake-cli'
            cli.write_text('#!' + sys.executable + '\nimport sys\nfor line in sys.stdin: print("你好你好")\n')
            cli.chmod(0o755)
            (root / 'lexicon.tsv').write_text('ㄋㄧˇ\t你\t0\nㄏㄠˇ\t好\t0\n')
            (root / 'test.txt').write_text('你好你好\n')
            result = subprocess.run([sys.executable, str(Path(__file__).with_name('evaluate.py')), '--jobs', '2',
                                     'error_analysis', '--cli', str(cli), '--lexicon', str(root / 'lexicon.tsv'),
                                     '--test', str(root / 'test.txt'), '-n', '1'], capture_output=True, text=True, check=True)
            self.assertIn('100.0%', result.stdout)

    def test_failure_is_not_silently_accepted(self):
        with self.assertRaises(subprocess.CalledProcessError):
            parallel_run(subprocess.run, [sys.executable, '-c', 'raise SystemExit(3)'], 2, threading.Semaphore(2),
                         input='a\nb\n', capture_output=True, text=True, check=True)

    def test_returncode_when_check_false(self):
        result = parallel_run(subprocess.run, [sys.executable, '-c', 'raise SystemExit(3)'], 2, threading.Semaphore(2),
                              input='a\nb\n', capture_output=True, text=True)
        self.assertEqual(result.returncode, 3)

if __name__ == '__main__':
    unittest.main()
