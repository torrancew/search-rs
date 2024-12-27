use crate::xapian::Stopper;

use std::{collections::HashSet, str::FromStr};

use stopwords::{Language, Stopwords};

pub struct StopList(HashSet<String>);

impl StopList {
    pub fn for_language(lang: &str) -> Option<Self> {
        Language::from_str(lang)
            .ok()
            .and_then(|l| stopwords::Spark::stopwords(l))
            .map(Self::from_iter)
    }
}

impl<S: AsRef<str>> FromIterator<S> for StopList {
    fn from_iter<T: IntoIterator<Item = S>>(iter: T) -> Self {
        Self(
            iter.into_iter()
                .map(|s| s.as_ref().to_lowercase())
                .collect(),
        )
    }
}

impl Stopper for StopList {
    fn is_stopword(&self, word: &str) -> bool {
        self.0.contains(&word.to_lowercase())
    }
}
