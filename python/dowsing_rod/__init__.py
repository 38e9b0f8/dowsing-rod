"""Dowsing Rod — Structural refactoring intelligence for Python.

Usage:
    import dowsing_rod
    result = dowsing_rod.scan(".")
    print(dowsing_rod.__version__)
"""

from dowsing_rod._native import scan, version, clear_cache

__version__ = version()
__all__ = ["scan", "version", "clear_cache", "__version__"]


def _cli_main():
    """Entry point for the `dowsing-rod` console script."""
    from dowsing_rod._native import cli_main
    cli_main()
