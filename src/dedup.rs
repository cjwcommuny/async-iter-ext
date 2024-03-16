use std::pin::Pin;
use std::task::{Context, Poll};

use futures::{Stream, StreamExt};
use pin_project::pin_project;

#[pin_project]
struct DedupReturnNone<S, T> {
    #[pin]
    stream: S,

    prev: Option<T>,
}

impl<S> Stream for DedupReturnNone<S, S::Item>
where
    S: Stream,
    S::Item: PartialEq,
{
    type Item = Option<S::Item>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        use Poll::{Pending, Ready};

        let this = self.project();
        let current = this.stream.poll_next(cx);
        let Ready(current) = current else {
            return Pending;
        };
        let Some(current) = current else {
            return Ready(this.prev.take().map(Some));
        };
        let prev = take_if(this.prev, |prev| *prev != current);
        let _ = this.prev.insert(current);
        Ready(Some(prev))
    }
}

pub fn dedup<S>(stream: S) -> impl Stream<Item = S::Item>
where
    S: Stream,
    S::Item: PartialEq,
{
    DedupReturnNone { stream, prev: None }.filter_map(std::future::ready)
}

fn take_if<T, P>(this: &mut Option<T>, predicate: P) -> Option<T>
where
    P: FnOnce(&mut T) -> bool,
{
    if this.as_mut().map_or(false, predicate) {
        this.take()
    } else {
        None
    }
}

#[cfg(test)]
mod test {
    use futures::executor::block_on;
    use futures::stream;
    use futures::StreamExt;

    use super::dedup;

    #[test]
    fn test_dedup_basic() {
        let s = stream::iter([1, 1, 2, 3, 3, 3]);
        let s = dedup(s);
        let v: Vec<_> = block_on(s.collect());
        assert_eq!(vec![1, 2, 3], v)
    }

    #[test]
    fn test_dedup_no_dup() {
        let s = stream::iter([1, 2, 3]);
        let s = dedup(s);
        let v: Vec<_> = block_on(s.collect());
        assert_eq!(vec![1, 2, 3], v)
    }

    #[test]
    fn test_dedup_singleton() {
        let s = stream::iter([1]);
        let s = dedup(s);
        let v: Vec<_> = block_on(s.collect());
        assert_eq!(vec![1], v)
    }

    #[test]
    fn test_dedup_empty() {
        let s = stream::iter(Vec::<i32>::new());
        let s = dedup(s);
        let v: Vec<_> = block_on(s.collect());
        assert_eq!(Vec::<i32>::new(), v)
    }
}
