//! The one owner of network reads in the binary: `sivi fonts install` fetches each file through it, bounded and reported, and nothing else reaches the network.

use std::fmt;
use std::time::Duration;

/// How long one fetch may take in all.
const TIMEOUT: Duration = Duration::from_secs(300);
/// How the binary names itself to the server.
const USER_AGENT: &str = concat!("sivi/", env!("CARGO_PKG_VERSION"));
const NOT_FOUND: u16 = 404;

mod raw {
    #![expect(
        clippy::disallowed_methods,
        reason = "the binary's one network owner builds its agent and performs each fetch here"
    )]

    /// The first `limit` bytes at `url`, or the transport or status error.
    pub(super) fn fetch(url: &str, limit: u64) -> Result<Vec<u8>, ureq::Error> {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(super::TIMEOUT))
            .user_agent(super::USER_AGENT)
            .build();
        let agent: ureq::Agent = config.into();
        let mut response = agent.get(url).call()?;
        response.body_mut().with_config().limit(limit).read_to_vec()
    }
}

/// Why a fetch did not return the bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FetchErrorKind {
    /// The server has no such file.
    NotFound,
    /// The server answered with another failing status.
    Status(u16),
    /// The body passed the limit.
    TooLarge(u64),
    /// The connection or the transfer failed.
    Transport(String),
}

/// A fetch that did not return the bytes, with the URL it asked for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FetchError {
    pub url: String,
    pub kind: FetchErrorKind,
}

impl fmt::Display for FetchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            FetchErrorKind::NotFound => write!(formatter, "{} is not there", self.url),
            FetchErrorKind::Status(status) => {
                write!(formatter, "{} answered with status {status}", self.url)
            }
            FetchErrorKind::TooLarge(limit) => {
                write!(formatter, "{} holds more than {limit} bytes", self.url)
            }
            FetchErrorKind::Transport(message) => {
                write!(formatter, "{} could not be fetched: {message}", self.url)
            }
        }
    }
}

/// A source of bytes by URL, which the web provides and the tests fake.
pub trait Fetch {
    /// The first `limit` bytes at `url`.
    ///
    /// # Errors
    ///
    /// Returns why the bytes did not arrive.
    fn fetch(&mut self, url: &str, limit: u64) -> Result<Vec<u8>, FetchError>;
}

/// The web, through the one network owner.
#[derive(Clone, Copy, Debug, Default)]
pub struct Web;

impl Fetch for Web {
    fn fetch(&mut self, url: &str, limit: u64) -> Result<Vec<u8>, FetchError> {
        raw::fetch(url, limit).map_err(|error| {
            let kind = if let ureq::Error::StatusCode(status) = &error {
                if *status == NOT_FOUND {
                    FetchErrorKind::NotFound
                } else {
                    FetchErrorKind::Status(*status)
                }
            } else if let ureq::Error::BodyExceedsLimit(bytes) = &error {
                FetchErrorKind::TooLarge(*bytes)
            } else {
                FetchErrorKind::Transport(error.to_string())
            };
            FetchError {
                url: String::from(url),
                kind,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{FetchError, FetchErrorKind};

    #[test]
    fn each_failure_names_the_url_and_its_cause() {
        let url = String::from("https://example.invalid/x.ttf");
        let not_found = FetchError {
            url: url.clone(),
            kind: FetchErrorKind::NotFound,
        };
        assert_eq!(
            not_found.to_string(),
            "https://example.invalid/x.ttf is not there"
        );
        let status = FetchError {
            url: url.clone(),
            kind: FetchErrorKind::Status(500),
        };
        assert!(status.to_string().ends_with("answered with status 500"));
        let large = FetchError {
            url: url.clone(),
            kind: FetchErrorKind::TooLarge(10),
        };
        assert!(large.to_string().ends_with("holds more than 10 bytes"));
        let transport = FetchError {
            url,
            kind: FetchErrorKind::Transport(String::from("refused")),
        };
        assert!(
            transport
                .to_string()
                .ends_with("could not be fetched: refused")
        );
    }
}
