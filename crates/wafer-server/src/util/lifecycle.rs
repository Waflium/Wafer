#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Lifecycle {
    #[default]
    Stable,
    Experimental,
}

impl Lifecycle {
    pub fn add(self, other: Lifecycle) -> Lifecycle {
        if self == Lifecycle::Experimental || other == Lifecycle::Experimental { Lifecycle::Experimental } else { Lifecycle::Stable }
    }
}

impl std::fmt::Display for Lifecycle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Lifecycle::Stable => "Stable",
            Lifecycle::Experimental => "Experimental",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Lifecycle;

    #[test]
    fn experimental_wins_when_added() {
        assert_eq!(Lifecycle::Stable.add(Lifecycle::Stable), Lifecycle::Stable);
        assert_eq!(Lifecycle::Stable.add(Lifecycle::Experimental), Lifecycle::Experimental);
        assert_eq!(Lifecycle::Experimental.add(Lifecycle::Stable), Lifecycle::Experimental);
        assert_eq!(Lifecycle::Stable.to_string(), "Stable");
        assert_eq!(Lifecycle::Experimental.to_string(), "Experimental");
    }
}
