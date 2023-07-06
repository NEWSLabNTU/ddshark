use anyhow::{bail, ensure, Result};
use intrusive_collections::{intrusive_adapter, Bound, KeyAdapter, RBTree, RBTreeLink};
use std::ops::Range;

#[derive(Debug)]
pub struct DefragBuf {
    len: usize,
    intervals: RBTree<Adapter>,
}

#[derive(Debug)]
struct Node {
    link: RBTreeLink,
    start: usize,
    kind: IntervalKind,
}

impl Node {
    fn new(start: usize, kind: IntervalKind) -> Box<Self> {
        Box::new(Self {
            link: RBTreeLink::new(),
            start,
            kind,
        })
    }
}

intrusive_adapter!(Adapter  = Box<Node>: Node { link: RBTreeLink });

impl<'a> KeyAdapter<'a> for Adapter {
    type Key = usize;

    fn get_key(
        &self,
        value: &'a <Self::PointerOps as intrusive_collections::PointerOps>::Value,
    ) -> Self::Key {
        value.start
    }
}

impl DefragBuf {
    pub fn new(len: usize) -> Self {
        let mut intervals = RBTree::new(Adapter::new());
        if len == 0 {
            return Self { len, intervals };
        }

        intervals.insert(Node::new(0, IntervalKind::Free));
        intervals.insert(Node::new(len, IntervalKind::Used));

        Self { len, intervals }
    }

    pub fn insert(&mut self, range: Range<usize>) -> Result<()> {
        let start = range.start;
        let end = range.end;
        ensure!(start < end, "invalid range");
        ensure!(
            end <= self.len,
            "range.end() must not exceed the size limit"
        );

        let mut cursor = self.intervals.upper_bound_mut(Bound::Included(&start));
        let Some(curr) = cursor.get() else {
            bail!("must insert within a free interval");
        };
        ensure!(
            curr.kind == IntervalKind::Free,
            "must insert within a free interval"
        );

        let next = cursor.peek_next().get().unwrap();
        ensure!(end <= next.start, "must insert within a free interval");

        match (start == curr.start, end == next.start) {
            (true, true) => {
                cursor.remove();
                cursor.remove();
            }
            (true, false) => {
                let result = cursor.replace_with(Node::new(end, IntervalKind::Free));
                assert!(result.is_ok());
            }
            (false, true) => {
                cursor.move_next();
                let result = cursor.replace_with(Node::new(start, IntervalKind::Used));
                assert!(result.is_ok());
            }
            (false, false) => {
                cursor.move_next();
                cursor.insert_before(Node::new(start, IntervalKind::Used));
                cursor.insert_before(Node::new(end, IntervalKind::Free));
            }
        }

        Ok(())
    }

    pub fn is_full(&self) -> bool {
        self.intervals.front().is_null()
    }

    #[cfg(test)]
    pub(crate) fn free_intervals(&self) -> impl Iterator<Item = Range<usize>> + '_ {
        let mut cursor = self.intervals.front();
        if let Some(front) = cursor.get() {
            assert!(front.kind == IntervalKind::Free);
        }

        std::iter::from_fn(move || {
            let first = cursor.get()?;
            let start = first.start;

            cursor.move_next();
            let second = cursor.get().unwrap();
            let end = second.start;

            cursor.move_next();

            Some(start..end)
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum IntervalKind {
    Free,
    Used,
}

#[cfg(test)]
mod tests {
    use super::DefragBuf;

    #[test]
    fn defrag_buf_test() {
        {
            let mut buf = DefragBuf::new(10);

            let free_invs: Vec<_> = buf.free_intervals().collect();
            assert_eq!(free_invs, vec![0..10]);

            buf.insert(0..10).unwrap();
            let free_invs: Vec<_> = buf.free_intervals().collect();
            assert!(free_invs.is_empty());
        }

        {
            let mut buf = DefragBuf::new(10);
            buf.insert(1..6).unwrap();
            assert!(buf.insert(3..4).is_err());
            let free_invs: Vec<_> = buf.free_intervals().collect();
            assert_eq!(free_invs, vec![0..1, 6..10]);
        }

        {
            let mut buf = DefragBuf::new(10);

            buf.insert(1..2).unwrap();
            let free_invs: Vec<_> = buf.free_intervals().collect();
            assert_eq!(free_invs, vec![0..1, 2..10]);

            buf.insert(2..4).unwrap();
            let free_invs: Vec<_> = buf.free_intervals().collect();
            assert_eq!(free_invs, vec![0..1, 4..10]);

            buf.insert(9..10).unwrap();
            let free_invs: Vec<_> = buf.free_intervals().collect();
            assert_eq!(free_invs, vec![0..1, 4..9]);

            buf.insert(6..7).unwrap();
            let free_invs: Vec<_> = buf.free_intervals().collect();
            assert_eq!(free_invs, vec![0..1, 4..6, 7..9]);

            buf.insert(8..9).unwrap();
            let free_invs: Vec<_> = buf.free_intervals().collect();
            assert_eq!(free_invs, vec![0..1, 4..6, 7..8]);

            buf.insert(4..6).unwrap();
            let free_invs: Vec<_> = buf.free_intervals().collect();
            assert_eq!(free_invs, vec![0..1, 7..8]);

            buf.insert(0..1).unwrap();
            let free_invs: Vec<_> = buf.free_intervals().collect();
            assert_eq!(free_invs, vec![7..8]);

            buf.insert(7..8).unwrap();
            let free_invs: Vec<_> = buf.free_intervals().collect();
            assert!(free_invs.is_empty());
        }
    }
}
