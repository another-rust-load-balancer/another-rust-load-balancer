/// This is a stable alternative to rust's unstable feature [str_split_once](https://github.com/rust-lang/rust/issues/74773).
pub fn split_once(string: &str, pattern: char) -> Option<(&str, &str)> {
  let mut splitter = string.splitn(2, pattern);
  let first = splitter.next()?;
  let second = splitter.next()?;
  Some((first, second))
}
