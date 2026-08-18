# Fixture: call-site shapes.
foo(1, 2)
obj.foo()
pkg.mod.fn()
ClassName()
registry["key"]()      # subscript callee: skipped, not guessed
make_fn()()            # call-of-call: outer skipped, inner recorded
