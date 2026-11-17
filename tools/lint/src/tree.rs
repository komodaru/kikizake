//! A file's path in the repository.
/// A file's path relative to the repository, '/'-separated, as `git ls-files` prints it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rel(pub String);

impl Rel {
    pub fn new(s: &str) -> Rel {
        Rel(s.to_owned())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    /// tests/, benches/, *_tests.rs, tests.rs: free to cast, index and unwrap
    pub fn is_test_file(&self) -> bool {
        let s = self.as_str();
        s.starts_with("tests/") || s.starts_with("benches/") || s.contains("/tests/") || s.contains("/benches/") || s.ends_with("_tests.rs") || s.ends_with("_test.rs") || s == "tests.rs" || s == "test.rs" || s.ends_with("/tests.rs") || s.ends_with("/test.rs")
    }
}

impl std::fmt::Display for Rel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

