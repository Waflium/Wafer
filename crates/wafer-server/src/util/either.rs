pub enum Either<L, R> {
    Left(L),
    Right(R),
}

impl<L, R> Either<L, R> {
    pub fn right(self) -> Option<R> {
        match self {
            Either::Left(_) => None,
            Either::Right(right) => Some(right),
        }
    }
}
