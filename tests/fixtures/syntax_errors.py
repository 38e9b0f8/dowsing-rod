# Intentionally broken Python — tests graceful error handling.

def this_is_fine():
    return 42

def broken_function(
    # missing closing paren and colon
