# 0.1.2

- Fix bugs in implementation of SharedCell::new(), get(), and replace().

# 0.1.1

- Fix unsound usage of `*mut Node<()>` to access members of a `Node<T>`.
- Fix memory ordering in `SharedCell::replace()`.

# 0.1.0

- First release.
