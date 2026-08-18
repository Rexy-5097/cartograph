// Fixture: call-site shapes.
declare function foo(a: number, b: number): void;
declare const console: { log(msg: string): void };
declare const a: { b: { c(): void } };
declare class Foo { constructor(n: number); }
declare const arr: Array<() => void>;

foo(1, 2);
console.log("hello");
a.b.c();
new Foo(42);
this_is_fine();
arr[0]();          // computed callee: skipped, not guessed
(function () {})(); // IIFE: skipped, not guessed
