pub mod modules;

pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

pub fn hello(name: &str) -> String {
    format!("Hello {name} from purgent-core v{}!", version())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_returns_non_empty() {
        assert!(!version().is_empty());
    }

    #[test]
    fn hello_contains_name_and_version() {
        let result = hello("Test");
        assert!(result.contains("Test"));
        assert!(result.contains(&version()));
    }
}
