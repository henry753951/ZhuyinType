"""Run unchanged golden-vector/parity/latency validation against the new tree."""
from common import ROOT, v3


def main():
    v3()
    import common
    common.OUT = ROOT/'data/build/v3_homophone'
    import validate
    validate.main()


if __name__=='__main__':
    main()
