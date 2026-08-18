# Fixture: a broken region must not destroy the rest of the file.
from good import thing


def fine():
    pass


def broken(:
    pass


def also_fine():
    pass


AFTER = "/still/extracted"
