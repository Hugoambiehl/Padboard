//! Pages of the user guides the app links to. Built here, never taken from
//! the UI: the page only names a topic.

use serde::Deserialize;

/// The user guides, as published in the repository.
const DOCS: &str = "https://github.com/Hugoambiehl/Padboard/blob/main/docs";

/// What the user needs help with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HelpTopic {
    /// An antivirus blocked the installer of an update.
    Antivirus,
}

impl HelpTopic {
    #[cfg(test)]
    const ALL: &[Self] = &[Self::Antivirus];

    /// The page answering it.
    #[must_use]
    pub fn url(self) -> String {
        let page = match self {
            Self::Antivirus => "install-windows.md#if-your-antivirus-blocks-padboard",
        };
        format!("{DOCS}/{page}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The anchors are the headings of docs/install-windows.md.
    #[test]
    fn help_pages_exist() {
        let guide = include_str!("../../../../docs/install-windows.md");
        for topic in HelpTopic::ALL {
            let url = topic.url();
            let anchor = url.rsplit_once('#').unwrap().1;
            let found = guide.lines().any(|line| {
                line.strip_prefix("## ").is_some_and(|heading| {
                    heading
                        .to_lowercase()
                        .chars()
                        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
                        .collect::<String>()
                        .replace(' ', "-")
                        == anchor
                })
            });
            assert!(found, "no heading for #{anchor} in install-windows.md");
        }
    }
}
