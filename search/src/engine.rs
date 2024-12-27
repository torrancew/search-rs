use thiserror::Error;

use crate::{prelude::*, xapian::DbAction};

use std::{borrow::Borrow, cell::RefCell, marker::PhantomData, path::Path};

pub struct Indexer<S> {
    db: xapian::WritableDatabase,
    termgen: xapian::TermGenerator,
    _schema: PhantomData<S>,
}

#[derive(Debug, Error)]
pub enum DbError {}

#[derive(Debug, Error)]
pub enum QueryParserError {}

#[derive(Debug, Error)]
pub enum SearchError {
    #[error("database error: {0}")]
    Database(DbError),
    #[error("query parser error: {0}")]
    QueryParser(QueryParserError),
}

impl<S: Schema> Indexer<S> {
    pub fn create(path: impl AsRef<Path>) -> Result<Self, DbError> {
        Self::open_with_mode(path, DbAction::Create)
    }

    pub fn create_or_open(path: impl AsRef<Path>) -> Result<Self, DbError> {
        Self::open_with_mode(path, DbAction::CreateOrOpen)
    }

    pub fn create_or_overwrite(path: impl AsRef<Path>) -> Result<Self, DbError> {
        Self::open_with_mode(path, DbAction::CreateOrOverwrite)
    }

    pub fn inmemory() -> Result<Self, DbError> {
        Self::open_with_mode::<&Path>(None, None)
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, DbError> {
        Self::open_with_mode(path, DbAction::Open)
    }

    fn open_with_mode<P: AsRef<Path>>(
        path: impl Into<Option<P>>,
        mode: impl Into<Option<xapian::DbAction>>,
    ) -> Result<Self, DbError> {
        let db = match path.into() {
            Some(path) => xapian::WritableDatabase::open(path, mode, None, None, None),
            None => xapian::WritableDatabase::inmemory(),
        };
        let mut termgen = xapian::TermGenerator::default();
        termgen.set_database(&db);

        Ok(Indexer {
            db,
            termgen,
            _schema: Default::default(),
        })
    }

    pub fn batch_index<T: Borrow<S>>(&mut self, items: impl IntoIterator<Item = T>) {
        self.batch_index_and_then(items, |_| ());
    }

    pub fn batch_index_and_then<T: Borrow<S>>(
        &mut self,
        items: impl IntoIterator<Item = T>,
        f: impl Fn((&mut xapian::Document, &S)),
    ) {
        self.db.begin_transaction(None);
        for i in items {
            let item = i.borrow();
            let mut doc = item.index(&mut self.termgen);
            f((&mut doc, item));
            self.db.add_document(doc);
        }
        self.db.commit_transaction();
    }

    pub fn commit(&mut self) {
        self.db.commit()
    }

    pub fn index(&mut self, item: impl Borrow<S>) {
        self.index_and_then(item, |_| ())
    }

    pub fn index_and_then(
        &mut self,
        item: impl Borrow<S>,
        f: impl Fn((&mut xapian::Document, &S)),
    ) {
        let item = item.borrow();
        let mut doc = item.index(&mut self.termgen);
        f((&mut doc, item));
        self.db.add_document(doc);
    }

    pub fn search(
        &self,
        query: impl AsRef<str>,
        page_size: impl Into<Option<u32>>,
        max_docs: impl Into<Option<u32>>,
    ) -> Result<Search<S>, SearchError> {
        Searcher::from(self).search(query, page_size, max_docs)
    }
}

pub struct Search<S> {
    enquire: xapian::Enquire,
    page_size: u32,
    max_docs: u32,
    _schema: PhantomData<S>,
}

impl<S: Schema> Search<S> {
    fn new(enquire: xapian::Enquire, page_size: u32, max_docs: u32) -> Self {
        Self {
            enquire,
            page_size,
            max_docs,
            _schema: Default::default(),
        }
    }

    pub fn update(&mut self, query: impl AsRef<str>) {
        self.enquire.set_query(
            S::query_parser().parse_query::<&str>(query, None, None),
            None,
        )
    }

    pub fn results(&self, page: u32) -> xapian::MSet {
        let first = page * self.page_size;
        self.enquire
            .mset(first, self.page_size, self.max_docs, None)
    }
}

pub struct Searcher<S> {
    db: xapian::Database,
    parser: RefCell<xapian::QueryParser>,
    _schema: PhantomData<S>,
}

impl<S: Schema> From<Indexer<S>> for Searcher<S> {
    fn from(value: Indexer<S>) -> Self {
        Self::from(&value)
    }
}

impl<S: Schema> From<&Indexer<S>> for Searcher<S> {
    fn from(value: &Indexer<S>) -> Self {
        let db = value.db.read_only();
        let parser = RefCell::from(S::query_parser());

        Self {
            db,
            parser,
            _schema: Default::default(),
        }
    }
}

impl<S: Schema> Searcher<S> {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SearchError> {
        let db = xapian::Database::open(path, None);
        let parser = RefCell::from(S::query_parser());

        Ok(Self {
            db,
            parser,
            _schema: Default::default(),
        })
    }

    pub fn search(
        &self,
        query: impl AsRef<str>,
        page_size: impl Into<Option<u32>>,
        max_docs: impl Into<Option<u32>>,
    ) -> Result<Search<S>, SearchError> {
        let mut enquire = xapian::Enquire::new(&self.db);
        enquire.set_query(
            self.parser
                .borrow_mut()
                .parse_query::<&str>(query, None, None),
            None,
        );

        Ok(Search::new(
            enquire,
            page_size.into().unwrap_or(100),
            max_docs.into().unwrap_or(self.db.doc_count()),
        ))
    }
}
