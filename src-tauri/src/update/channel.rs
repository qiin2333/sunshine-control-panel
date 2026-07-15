#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReleaseChannel {
    Stable,
    IncludePrerelease,
    Prerelease,
}

impl ReleaseChannel {
    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Stable => "稳定版",
            Self::IncludePrerelease => "稳定版及预发布版",
            Self::Prerelease => "预发布版",
        }
    }

    pub(crate) fn matches(self, prerelease: bool) -> bool {
        match self {
            Self::Stable => !prerelease,
            Self::IncludePrerelease => true,
            Self::Prerelease => prerelease,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_requested_release_kind() {
        assert!(ReleaseChannel::IncludePrerelease.matches(true));
        assert!(ReleaseChannel::IncludePrerelease.matches(false));
        assert!(ReleaseChannel::Stable.matches(false));
        assert!(!ReleaseChannel::Stable.matches(true));
        assert!(ReleaseChannel::Prerelease.matches(true));
        assert!(!ReleaseChannel::Prerelease.matches(false));
    }
}
