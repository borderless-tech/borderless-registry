use borderless_pkg::semver::SemVer;
use std::net::SocketAddr;
use std::str::FromStr;
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid format: the identifier format is invalid")]
    InvalidFormat,
    #[error("Missing repository or tag: repository and tag are required")]
    MissingRepoOrTag,
    #[error("Missing namespace: namespace is required")]
    MissingNamespace,
}

/// The Tag can be a keyword like latest, nightly etc.
/// if not the tag describe a version in SemVer format.
#[derive(Debug, Clone, PartialEq)]
pub enum Tag {
    Keyword(String),
    Version(SemVer),
}

impl FromStr for Tag {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let keywords = vec!["latest", "nightly"];
        if keywords.contains(&s) {
            Ok(Tag::Keyword(s.to_string()))
        } else {
            let version = SemVer::from_str(s).map_err(|_| Error::InvalidFormat)?;
            Ok(Tag::Version(version))
        }
    }
}

impl ToString for Tag {
    fn to_string(&self) -> String {
        match self {
            Tag::Keyword(s) => s.clone(),
            Tag::Version(v) => v.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Registry {
    Url(Url),
    SocketAddr(SocketAddr),
}

impl FromStr for Registry {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // First try to parse as socket address
        if let Ok(sock_addr) = SocketAddr::from_str(s) {
            return Ok(Registry::SocketAddr(sock_addr));
        }

        // Then try as URL
        let mut uri = s.to_string();

        // Add https:// if no scheme is present
        if !s.contains("://") {
            uri = format!("https://{}", uri);
        }

        match Url::parse(&uri) {
            Ok(url) => Ok(Registry::Url(url)),
            Err(_) => Err(Error::InvalidFormat),
        }
    }
}

impl ToString for Registry {
    fn to_string(&self) -> String {
        match self {
            Registry::Url(url) => {
                // For URLs that were auto-prefixed with https://,
                // we might want to return just the host:port format
                // This is a design decision - keeping full URL for now
                url.to_string()
            }
            Registry::SocketAddr(sock) => sock.to_string(),
        }
    }
}

/// The OciIdentifier describes a set of strings to identify an image in the registry
/// by the registry itself, the namespace of a given user, the repository and a version tag.
///
/// Format: [registry/]namespace[/additional/path]/repository:tag
/// - registry: Optional registry URL or socket address
/// - namespace: Required namespace (can contain multiple path segments)
/// - repository: Required repository name
/// - tag: Required tag (keyword or semantic version)
#[derive(Debug, Clone, PartialEq)]
pub struct OciIdentifier {
    pub registry: Option<Registry>,
    pub namespace: String,
    pub repository: String,
    pub tag: Tag,
}

impl FromStr for OciIdentifier {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Trim whitespace
        let s = s.trim();

        if s.is_empty() {
            return Err(Error::InvalidFormat);
        }

        // Handle URLs specially - they start with a protocol
        let (registry, remaining_path) = if s.contains("://") {
            // Find where the registry URL ends and the path begins
            // Look for the third slash (after protocol://)
            let protocol_end = s.find("://").unwrap() + 3; // Skip past "://"
            let after_protocol = &s[protocol_end..];

            if let Some(path_start) = after_protocol.find('/') {
                let registry_part = &s[..protocol_end + path_start];
                let path_part = &s[protocol_end + path_start + 1..]; // Skip the '/'

                match Registry::from_str(registry_part) {
                    Ok(reg) => (Some(reg), path_part),
                    Err(_) => (None, s), // Treat whole thing as path if registry parsing fails
                }
            } else {
                return Err(Error::InvalidFormat); // URL with no path
            }
        } else {
            (None, s)
        };

        // Now parse the remaining path
        let mut parts: Vec<&str> = remaining_path.split('/').collect();

        if parts.len() < 2 {
            return Err(Error::InvalidFormat);
        }

        // Extract repository and tag from the last segment
        let repo_and_tag = parts.pop().ok_or(Error::MissingRepoOrTag)?;

        // Validate colon format
        if repo_and_tag.starts_with(':') || repo_and_tag.ends_with(':') {
            return Err(Error::InvalidFormat);
        }

        let repo_tag_parts: Vec<&str> = repo_and_tag.split(':').collect();
        if repo_tag_parts.len() != 2 {
            return Err(Error::InvalidFormat);
        }

        let repository = repo_tag_parts[0].to_string();
        let tag_str = repo_tag_parts[1];

        if repository.is_empty() || tag_str.is_empty() {
            return Err(Error::InvalidFormat);
        }

        // Parse tag - try SemVer first, fall back to keyword
        let tag = match SemVer::from_str(tag_str) {
            Ok(version) => Tag::Version(version),
            Err(_) => Tag::Keyword(tag_str.to_string()),
        };

        // Handle non-URL registries if we don't already have one
        let final_registry = if registry.is_none() && !parts.is_empty() {
            let potential_registry = parts[0];

            // Only try to parse as registry if it looks like a registry:
            // - Contains a port (e.g., "127.0.0.1:5000", "localhost:5000")
            // - Contains a domain with TLD (e.g., "gcr.io", "registry.example.com")
            let looks_like_registry = potential_registry.contains(':')
                || (potential_registry.contains('.')
                    && !potential_registry.starts_with('.')
                    && !potential_registry.ends_with('.'));

            if looks_like_registry {
                if let Ok(reg) = Registry::from_str(potential_registry) {
                    // Remove the registry part from parts
                    parts.remove(0);
                    Some(reg)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            registry
        };

        // Everything remaining is the namespace
        if parts.is_empty() {
            return Err(Error::MissingNamespace);
        }

        let namespace = parts.join("/");

        Ok(Self {
            registry: final_registry,
            namespace,
            repository,
            tag,
        })
    }
}

impl ToString for OciIdentifier {
    fn to_string(&self) -> String {
        let mut result = String::new();

        // Add registry if present
        if let Some(registry) = &self.registry {
            result.push_str(&registry.to_string());
            result.push('/');
        }

        // Add namespace
        result.push_str(&self.namespace);
        result.push('/');

        // Add repository
        result.push_str(&self.repository);
        result.push(':');

        // Add tag
        result.push_str(&self.tag.to_string());

        result
    }
}

// Helper methods for OciIdentifier
impl OciIdentifier {
    /// Create a new OciIdentifier with the minimum required fields
    pub fn new(namespace: String, repository: String, tag: Tag) -> Self {
        Self {
            registry: None,
            namespace,
            repository,
            tag,
        }
    }

    /// Create a new OciIdentifier with a registry
    pub fn with_registry(
        registry: Registry,
        namespace: String,
        repository: String,
        tag: Tag,
    ) -> Self {
        Self {
            registry: Some(registry),
            namespace,
            repository,
            tag,
        }
    }

    /// Get the full repository path (namespace + repository)
    pub fn full_repository_path(&self) -> String {
        format!("{}/{}", self.namespace, self.repository)
    }

    /// Check if this identifier has a registry
    pub fn has_registry(&self) -> bool {
        self.registry.is_some()
    }

    /// Get the registry host if it's a URL
    pub fn registry_host(&self) -> Option<String> {
        match &self.registry {
            Some(Registry::Url(url)) => url.host_str().map(|h| h.to_string()),
            Some(Registry::SocketAddr(addr)) => Some(addr.ip().to_string()),
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    // ===== SUCCESSFUL PARSING TESTS =====

    #[test]
    fn parse_basic_oci_identifier() -> Result<(), Error> {
        let oci = OciIdentifier::from_str("nginx/nginx:latest")?;

        assert!(oci.registry.is_none());
        assert_eq!(oci.namespace, "nginx");
        assert_eq!(oci.repository, "nginx");
        assert_eq!(oci.tag, Tag::Keyword("latest".to_string()));

        Ok(())
    }

    #[test]
    fn parse_oci_with_semver_tag() -> Result<(), Error> {
        let oci = OciIdentifier::from_str("library/ubuntu:22.04.3")?;

        assert_eq!(oci.namespace, "library");
        assert_eq!(oci.repository, "ubuntu");
        assert!(matches!(oci.tag, Tag::Version(_)));

        Ok(())
    }

    #[test]
    fn parse_oci_with_url_registry() -> Result<(), Error> {
        let oci = OciIdentifier::from_str("https://registry.docker.io/library/postgres:15.2")?;

        assert!(oci.registry.is_some());
        assert!(matches!(oci.registry.as_ref().unwrap(), Registry::Url(_)));
        assert_eq!(oci.namespace, "library");
        assert_eq!(oci.repository, "postgres");

        Ok(())
    }

    #[test]
    fn parse_oci_with_socket_registry() -> Result<(), Error> {
        let oci = OciIdentifier::from_str("127.0.0.1:5000/myorg/myapp:nightly")?;

        assert!(oci.registry.is_some());
        if let Some(Registry::SocketAddr(addr)) = &oci.registry {
            assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
            assert_eq!(addr.port(), 5000);
        } else {
            panic!("Expected SocketAddr registry");
        }
        assert_eq!(oci.namespace, "myorg");
        assert_eq!(oci.repository, "myapp");

        Ok(())
    }

    #[test]
    fn parse_oci_with_hostname_registry() -> Result<(), Error> {
        let oci = OciIdentifier::from_str("gcr.io/google-containers/pause:3.9")?;

        assert!(oci.registry.is_some());
        assert_eq!(oci.namespace, "google-containers");
        assert_eq!(oci.repository, "pause");

        Ok(())
    }

    #[test]
    fn parse_oci_with_port_in_hostname() -> Result<(), Error> {
        let oci = OciIdentifier::from_str("registry.example.com:443/company/service:1.2.3")?;

        assert!(oci.registry.is_some());
        assert_eq!(oci.namespace, "company");
        assert_eq!(oci.repository, "service");

        Ok(())
    }

    #[test]
    fn parse_different_keyword_tags() -> Result<(), Error> {
        let test_cases = vec![
            ("user/app:latest", "latest"),
            ("user/app:nightly", "nightly"),
        ];

        for (input, expected_keyword) in test_cases {
            let oci = OciIdentifier::from_str(input)?;
            if let Tag::Keyword(keyword) = &oci.tag {
                assert_eq!(keyword, expected_keyword);
            } else {
                panic!("Expected keyword tag for {}", input);
            }
        }

        Ok(())
    }

    #[test]
    fn parse_with_whitespace() -> Result<(), Error> {
        // Test that whitespace is properly trimmed
        let oci = OciIdentifier::from_str("  nginx/nginx:latest  ")?;

        assert_eq!(oci.namespace, "nginx");
        assert_eq!(oci.repository, "nginx");
        assert_eq!(oci.tag, Tag::Keyword("latest".to_string()));

        Ok(())
    }

    // ===== ERROR HANDLING TESTS =====

    #[test]
    fn parse_empty_string_fails() {
        let result = OciIdentifier::from_str("");
        assert!(matches!(result, Err(Error::InvalidFormat)));
    }

    #[test]
    fn parse_whitespace_only_fails() {
        let result = OciIdentifier::from_str("   ");
        assert!(matches!(result, Err(Error::InvalidFormat)));
    }

    #[test]
    fn parse_missing_namespace_fails() {
        let result = OciIdentifier::from_str("repo:tag");
        assert!(matches!(result, Err(Error::InvalidFormat)));
    }

    #[test]
    fn parse_missing_tag_fails() {
        let result = OciIdentifier::from_str("namespace/repo");
        assert!(matches!(result, Err(Error::InvalidFormat)));
    }

    #[test]
    fn parse_missing_repository_fails() {
        let result = OciIdentifier::from_str("namespace/:tag");
        assert!(result.is_err());
    }

    #[test]
    fn parse_invalid_colon_format_fails() {
        let test_cases = vec![
            "namespace/repo:",          // No tag after colon
            "namespace/repo::",         // Double colon
            "namespace/repo:tag:extra", // Too many colons
        ];

        for case in test_cases {
            let result = OciIdentifier::from_str(case);
            assert!(result.is_err(), "Should fail for: {}", case);
        }
    }

    // ===== REAL-WORLD EXAMPLES =====

    #[test]
    fn parse_real_world_examples() -> Result<(), Error> {
        let examples = vec![
            // Docker Hub
            "library/nginx:1.21.6",
            "library/node:18-alpine",
            // Google Container Registry
            "gcr.io/google-containers/pause:3.9",
            // AWS ECR
            "123456789012.dkr.ecr.us-west-2.amazonaws.com/my-org/my-app:latest",
            // Azure Container Registry
            "myregistry.azurecr.io/samples/nginx:latest",
            // Harbor
            "harbor.example.com/library/redis:6.2",
            // Localhost with port
            "localhost:5000/myproject/myimage:dev",
        ];

        for example in examples {
            let result = OciIdentifier::from_str(example);
            assert!(result.is_ok(), "Failed to parse: {}", example);

            let oci = result?;
            assert!(!oci.namespace.is_empty());
            assert!(!oci.repository.is_empty());
        }

        Ok(())
    }

    // ===== EDGE CASES =====
    #[test]
    fn parse_deep_namespace_path() -> Result<(), Error> {
        let oci = OciIdentifier::from_str("registry.io/org/team/project/service:1.0.0")?;

        assert!(oci.registry.is_some());
        assert_eq!(oci.namespace, "org/team/project");
        assert_eq!(oci.repository, "service");

        Ok(())
    }

    #[test]
    fn parse_numeric_tags() -> Result<(), Error> {
        let oci = OciIdentifier::from_str("library/node:18")?;

        // "18" is not valid SemVer, should be treated as keyword
        assert!(matches!(oci.tag, Tag::Keyword(_)));

        Ok(())
    }

    // ===== ROUNDTRIP TESTS =====
    #[test]
    fn roundtrip_test() -> Result<(), Error> {
        let test_cases = vec![
            ("nginx/nginx:latest", "nginx/nginx:latest"),
            ("library/ubuntu:22.04.3", "library/ubuntu:22.4.3"), // SemVer normalizes this
            ("myorg/myapp:1.2.3", "myorg/myapp:1.2.3"),
            (
                "deep/namespace/path/repo:1.0.0",
                "deep/namespace/path/repo:1.0.0",
            ),
        ];

        for (original, expected_output) in test_cases {
            let oci = OciIdentifier::from_str(original)?;
            let serialized = oci.to_string();
            let reparsed = OciIdentifier::from_str(&serialized)?;

            assert_eq!(oci, reparsed, "Roundtrip failed for: {}", original);
            assert_eq!(
                serialized, expected_output,
                "String representation mismatch for: {}",
                original
            );
        }

        Ok(())
    }

    // ===== HELPER METHOD TESTS =====

    #[test]
    fn test_registry_detection_logic() -> Result<(), Error> {
        // These should NOT be treated as registries
        let non_registries = vec![
            "nginx/nginx:latest",
            "library/ubuntu:22.04.3",
            "user/app:latest",
        ];

        for input in non_registries {
            let oci = OciIdentifier::from_str(input)?;
            assert!(
                oci.registry.is_none(),
                "Should not detect registry for: {}",
                input
            );
        }

        // These SHOULD be treated as registries
        let with_registries = vec![
            "gcr.io/google-containers/pause:3.9",
            "127.0.0.1:5000/myorg/myapp:nightly",
            "https://registry.docker.io/library/postgres:15.2",
            "localhost:5000/myproject/myimage:dev",
            "registry.example.com:443/company/service:1.2.3",
        ];

        for input in with_registries {
            let oci = OciIdentifier::from_str(input)?;
            assert!(
                oci.registry.is_some(),
                "Should detect registry for: {}",
                input
            );
        }

        Ok(())
    }

    #[test]
    fn test_helper_methods() -> Result<(), Error> {
        let oci = OciIdentifier::from_str("gcr.io/google-containers/pause:3.9")?;

        assert!(oci.has_registry());
        assert_eq!(oci.full_repository_path(), "google-containers/pause");
        assert_eq!(oci.registry_host(), Some("gcr.io".to_string()));

        let simple_oci = OciIdentifier::from_str("nginx/nginx:latest")?;
        assert!(!simple_oci.has_registry());
        assert_eq!(simple_oci.registry_host(), None);

        Ok(())
    }

    // ===== CONSTRUCTOR TESTS =====
    #[test]
    fn test_constructors() {
        let tag = Tag::Keyword("latest".to_string());

        let oci = OciIdentifier::new("nginx".to_string(), "nginx".to_string(), tag.clone());

        assert!(oci.registry.is_none());
        assert_eq!(oci.namespace, "nginx");
        assert_eq!(oci.repository, "nginx");
        assert_eq!(oci.tag, tag);

        let registry = Registry::Url(Url::parse("https://gcr.io").unwrap());
        let oci_with_registry = OciIdentifier::with_registry(
            registry.clone(),
            "google-containers".to_string(),
            "pause".to_string(),
            Tag::Keyword("3.9".to_string()),
        );

        assert_eq!(oci_with_registry.registry, Some(registry));
    }
}
