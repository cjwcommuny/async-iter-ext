mod dedup;

use dedup::dedup_by;
pub use dedup::Dedup;
use futures::Stream;

pub trait StreamTools: Stream {
    fn dedup_by<F>(self, is_equal: F) -> Dedup<Self, Self::Item, F>
    where
        F: FnMut(&Self::Item, &Self::Item) -> bool,
        Self: Sized;
}

impl<S: Stream> StreamTools for S {
    fn dedup_by<F>(self, is_equal: F) -> Dedup<Self, Self::Item, F>
    where
        F: FnMut(&Self::Item, &Self::Item) -> bool,
    {
        dedup_by(self, is_equal)
    }
}
