# [retain_range](https://crates.io/crates/retain_range)

Extension of [`Vec::retain`](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.retain) to operate only on part of the vector defined by a
range. 

```rust
let mut vec = vec![1, 2, 3, 4, 5];
vec.retain_range(1..=3, |&x| x <= 2);
assert_eq!(vec, [1, 2, 5]);
```
