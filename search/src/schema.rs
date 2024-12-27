pub trait Schema {
    fn index(&self, indexer: &mut crate::xapian::TermGenerator) -> crate::xapian::Document;
    fn query_parser() -> crate::xapian::QueryParser;
}
