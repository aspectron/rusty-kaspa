use super::interval::Interval;
use super::{tree::*, *};
use crate::model;
use crate::model::stores::reachability::ReachabilityStoreReader;
use crate::model::{api::hash::Hash, stores::reachability::ReachabilityStore};

/// Init the reachability store to match the state required by the algorithmic layer.
/// The function first checks the store for possibly being initialized already.
pub fn init(store: &mut (impl ReachabilityStore + ?Sized)) -> Result<()> {
    init_with_params(store, model::ORIGIN, Interval::maximal())
}

pub(super) fn init_with_params(
    store: &mut (impl ReachabilityStore + ?Sized), origin: Hash, capacity: Interval,
) -> Result<()> {
    if store.has(origin)? {
        return Ok(());
    }
    store.insert(origin, Hash::ZERO, capacity, 0)?;
    store.set_reindex_root(origin)?;
    Ok(())
}

type HashIterator<'a> = &'a mut dyn Iterator<Item = Hash>;

/// Add a block to the DAG reachability data structures and persist using the provided `store`.
pub fn add_block(
    store: &mut (impl ReachabilityStore + ?Sized), new_block: Hash, selected_parent: Hash,
    mergeset_iterator: HashIterator,
) -> Result<()> {
    add_block_with_params(store, new_block, selected_parent, mergeset_iterator, None, None)
}

fn add_block_with_params(
    store: &mut (impl ReachabilityStore + ?Sized), new_block: Hash, selected_parent: Hash,
    mergeset_iterator: HashIterator, reindex_depth: Option<u64>, reindex_slack: Option<u64>,
) -> Result<()> {
    add_tree_block(
        store,
        new_block,
        selected_parent,
        reindex_depth.unwrap_or(crate::constants::perf::DEFAULT_REINDEX_DEPTH),
        reindex_slack.unwrap_or(crate::constants::perf::DEFAULT_REINDEX_SLACK),
    )?;
    add_dag_block(store, new_block, mergeset_iterator)?;
    Ok(())
}

fn add_dag_block(
    store: &mut (impl ReachabilityStore + ?Sized), new_block: Hash, mergeset_iterator: HashIterator,
) -> Result<()> {
    // Update the future covering set for blocks in the mergeset
    for merged_block in mergeset_iterator {
        insert_to_future_covering_set(store, merged_block, new_block)?;
    }
    Ok(())
}

fn insert_to_future_covering_set(
    store: &mut (impl ReachabilityStore + ?Sized), merged_block: Hash, new_block: Hash,
) -> Result<()> {
    match binary_search_descendant(
        store,
        store
            .get_future_covering_set(merged_block)?
            .as_slice(),
        new_block,
    )? {
        // We expect the query to not succeed, and to only return the correct insertion index.
        // The existences of a `future covering item` (`FCI`) which is a chain ancestor of `new_block`
        // contradicts `merged_block ∈ mergeset(new_block)`. Similarly, the existence of an FCI
        // which `new_block` is a chain ancestor of, contradicts processing order.
        SearchOutput::Found(_, _) => Err(ReachabilityError::DataInconsistency),
        SearchOutput::NotFound(i) => {
            store.insert_future_covering_item(merged_block, new_block, i)?;
            Ok(())
        }
    }
}

/// Hint to the reachability algorithm that `hint` is a candidate to become
/// the `virtual selected parent` (`VSP`). This might affect internal reachability heuristics such
/// as moving the reindex point. The consensus runtime is expected to call this function
/// for a new header selected tip which is `header only` / `pending UTXO verification`, or for a completely resolved `VSP`.
pub fn hint_virtual_selected_parent(store: &mut (impl ReachabilityStore + ?Sized), hint: Hash) -> Result<()> {
    try_advancing_reindex_root(
        store,
        hint,
        crate::constants::perf::DEFAULT_REINDEX_DEPTH,
        crate::constants::perf::DEFAULT_REINDEX_SLACK,
    )
}

/// Checks if the `anchor` block is a strict chain ancestor of the `queried` block (aka `anchor ∈ chain(queried)`).
/// Note that this results in `false` if `anchor == queried`
pub fn is_strict_chain_ancestor_of(
    store: &(impl ReachabilityStoreReader + ?Sized), anchor: Hash, queried: Hash,
) -> Result<bool> {
    Ok(store
        .get_interval(anchor)?
        .strictly_contains(store.get_interval(queried)?))
}

/// Checks if `anchor` block is a chain ancestor of `queried` block (aka `anchor ∈ chain(queried) ∪ {queried}`).
/// Note that we use the graph theory convention here which defines that a block is also an ancestor of itself.
pub fn is_chain_ancestor_of(
    store: &(impl ReachabilityStoreReader + ?Sized), anchor: Hash, queried: Hash,
) -> Result<bool> {
    Ok(store
        .get_interval(anchor)?
        .contains(store.get_interval(queried)?))
}

/// Returns true if `anchor` is a DAG ancestor of `queried` (aka `queried ∈ future(anchor) ∪ {anchor}`).
/// Note: this method will return true if `anchor == queried`.
/// The complexity of this method is O(log(|future_covering_set(anchor)|))
pub fn is_dag_ancestor_of(
    store: &(impl ReachabilityStoreReader + ?Sized), anchor: Hash, queried: Hash,
) -> Result<bool> {
    // First, check if `anchor` is a chain ancestor of queried
    if is_chain_ancestor_of(store, anchor, queried)? {
        return Ok(true);
    }
    // Otherwise, use previously registered future blocks to complete the
    // DAG reachability test
    match binary_search_descendant(store, store.get_future_covering_set(anchor)?.as_slice(), queried)? {
        SearchOutput::Found(_, _) => Ok(true),
        SearchOutput::NotFound(_) => Ok(false),
    }
}

/// Finds the child of `ancestor` which is also a chain ancestor of `descendant`.
pub fn get_next_chain_ancestor(
    store: &(impl ReachabilityStoreReader + ?Sized), descendant: Hash, ancestor: Hash,
) -> Result<Hash> {
    if descendant == ancestor {
        // The next ancestor does not exist
        return Err(ReachabilityError::BadQuery);
    }
    if !is_strict_chain_ancestor_of(store, ancestor, descendant)? {
        // `ancestor` isn't actually a chain ancestor of `descendant`, so by def
        // we cannot find the next ancestor as well
        return Err(ReachabilityError::BadQuery);
    }

    get_next_chain_ancestor_unchecked(store, descendant, ancestor)
}

/// Note: it is important to keep the unchecked version for internal module use,
/// since in some scenarios during reindexing `descendant` might have a modified
/// interval which was not propagated yet.
pub(super) fn get_next_chain_ancestor_unchecked(
    store: &(impl ReachabilityStoreReader + ?Sized), descendant: Hash, ancestor: Hash,
) -> Result<Hash> {
    match binary_search_descendant(store, store.get_children(ancestor)?.as_slice(), descendant)? {
        SearchOutput::Found(hash, i) => Ok(hash),
        SearchOutput::NotFound(i) => Err(ReachabilityError::BadQuery),
    }
}

enum SearchOutput {
    NotFound(usize), // `usize` is the position to insert at
    Found(Hash, usize),
}

fn binary_search_descendant(
    store: &(impl ReachabilityStoreReader + ?Sized), ordered_hashes: &[Hash], descendant: Hash,
) -> Result<SearchOutput> {
    if cfg!(debug_assertions) {
        // This is a linearly expensive assertion, keep it debug only
        assert_hashes_ordered(store, ordered_hashes);
    }

    // `Interval::end` represents the unique number allocated to this block
    let point = store.get_interval(descendant)?.end;

    // We use an `unwrap` here since otherwise we need to implement `binary_search`
    // ourselves, which is not worth the effort given that this would be an unrecoverable
    // error anyhow
    match ordered_hashes.binary_search_by_key(&point, |c| store.get_interval(*c).unwrap().start) {
        Ok(i) => Ok(SearchOutput::Found(ordered_hashes[i], i)),
        Err(i) => {
            // `i` is where `point` was expected (i.e., point < ordered_hashes[i].interval.start),
            // so we expect `ordered_hashes[i - 1].interval` to be the only candidate to contain `point`
            if i > 0 && is_chain_ancestor_of(store, ordered_hashes[i - 1], descendant)? {
                Ok(SearchOutput::Found(ordered_hashes[i - 1], i - 1))
            } else {
                Ok(SearchOutput::NotFound(i))
            }
        }
    }
}

fn assert_hashes_ordered(store: &(impl ReachabilityStoreReader + ?Sized), ordered_hashes: &[Hash]) {
    let intervals: Vec<Interval> = ordered_hashes
        .iter()
        .cloned()
        .map(|c| store.get_interval(c).unwrap())
        .collect();
    debug_assert!(intervals
        .as_slice()
        .windows(2)
        .all(|w| w[0].end < w[1].start))
}

/// Returns a forward iterator walking up the chain-selection tree from `from_ancestor`
/// to `to_descendant`, where `to_descendant` is included if `inclusive` is set to true.
/// The caller is expected to verify that `from_ancestor` is indeed a chain ancestor of
/// `to_descendant`, otherwise a `ReachabilityError::BadQuery` error will be returned.  
pub fn forward_chain_iterator(
    store: &(impl ReachabilityStoreReader + ?Sized), from_ancestor: Hash, to_descendant: Hash, inclusive: bool,
) -> impl Iterator<Item = Result<Hash>> + '_ {
    ForwardChainIterator::new(store, from_ancestor, to_descendant, inclusive)
}

/// Returns a backward iterator walking down the selected chain from `from_descendant`
/// to `to_ancestor`, where `to_ancestor` is included if `inclusive` is set to true.
/// The caller is expected to verify that `to_ancestor` is indeed a chain ancestor of
/// `from_descendant`, otherwise the iterator will eventually return an error.  
pub fn backward_chain_iterator(
    store: &(impl ReachabilityStoreReader + ?Sized), from_descendant: Hash, to_ancestor: Hash, inclusive: bool,
) -> impl Iterator<Item = Result<Hash>> + '_ {
    BackwardChainIterator::new(store, from_descendant, to_ancestor, inclusive)
}

/// Returns the default chain iterator, walking from `from` backward down the
/// selected chain until `virtual genesis` (aka `model::ORIGIN`; exclusive)
pub fn default_chain_iterator(
    store: &(impl ReachabilityStoreReader + ?Sized), from: Hash,
) -> impl Iterator<Item = Result<Hash>> + '_ {
    BackwardChainIterator::new(store, from, model::ORIGIN, false)
}

pub struct ForwardChainIterator<'a, T: ReachabilityStoreReader + ?Sized> {
    store: &'a T,
    current: Option<Hash>,
    descendant: Hash,
    inclusive: bool,
}

impl<'a, T: ReachabilityStoreReader + ?Sized> ForwardChainIterator<'a, T> {
    fn new(store: &'a T, from_ancestor: Hash, to_descendant: Hash, inclusive: bool) -> Self {
        Self { store, current: Some(from_ancestor), descendant: to_descendant, inclusive }
    }
}

impl<'a, T: ReachabilityStoreReader + ?Sized> Iterator for ForwardChainIterator<'a, T> {
    type Item = Result<Hash>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(current) = self.current {
            if current == self.descendant {
                if self.inclusive {
                    self.current = None;
                    Some(Ok(current))
                } else {
                    self.current = None;
                    None
                }
            } else {
                match get_next_chain_ancestor(self.store, self.descendant, current) {
                    Ok(next) => {
                        self.current = Some(next);
                        Some(Ok(current))
                    }
                    Err(e) => {
                        self.current = None;
                        Some(Err(e))
                    }
                }
            }
        } else {
            None
        }
    }
}

pub struct BackwardChainIterator<'a, T: ReachabilityStoreReader + ?Sized> {
    store: &'a T,
    current: Option<Hash>,
    ancestor: Hash,
    inclusive: bool,
}

impl<'a, T: ReachabilityStoreReader + ?Sized> BackwardChainIterator<'a, T> {
    fn new(store: &'a T, from_descendant: Hash, to_ancestor: Hash, inclusive: bool) -> Self {
        Self { store, current: Some(from_descendant), ancestor: to_ancestor, inclusive }
    }
}

impl<'a, T: ReachabilityStoreReader + ?Sized> Iterator for BackwardChainIterator<'a, T> {
    type Item = Result<Hash>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(current) = self.current {
            if current == self.ancestor {
                if self.inclusive {
                    self.current = None;
                    Some(Ok(current))
                } else {
                    self.current = None;
                    None
                }
            } else {
                debug_assert_ne!(current, Hash::ZERO);
                match self.store.get_parent(current) {
                    Ok(next) => {
                        self.current = Some(next);
                        Some(Ok(current))
                    }
                    Err(e) => {
                        self.current = None;
                        Some(Err(ReachabilityError::StoreError(e)))
                    }
                }
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use crate::{model::stores::reachability::MemoryReachabilityStore, processes::reachability::interval::Interval};

    #[test]
    fn test_add_tree_blocks() {
        // Arrange
        let mut store = MemoryReachabilityStore::new();

        // Act
        let root: Hash = 1.into();
        TreeBuilder::new(&mut store)
            .init_with_params(root, Interval::new(1, 15))
            .add_block(2.into(), root)
            .add_block(3.into(), 2.into())
            .add_block(4.into(), 2.into())
            .add_block(5.into(), 3.into())
            .add_block(6.into(), 5.into())
            .add_block(7.into(), 1.into())
            .add_block(8.into(), 6.into())
            .add_block(9.into(), 6.into())
            .add_block(10.into(), 6.into())
            .add_block(11.into(), 6.into());

        // Assert
        store.validate_intervals(root).unwrap();
    }

    #[test]
    fn test_add_early_blocks() {
        // Arrange
        let mut store = MemoryReachabilityStore::new();

        // Act
        let root: Hash = 1.into();
        let mut builder = TreeBuilder::new_with_params(&mut store, 2, 5);
        builder.init_with_params(root, Interval::maximal());
        for i in 2u64..100 {
            builder.add_block(i.into(), (i / 2).into());
        }

        // Should trigger an earlier than reindex root allocation
        builder.add_block(100.into(), 2.into());
        store.validate_intervals(root).unwrap();
    }

    #[test]
    fn test_add_dag_blocks() {
        // Arrange
        let mut store = MemoryReachabilityStore::new();

        // Act
        DagBuilder::new(&mut store)
            .init()
            .add_block(DagBlock::new(1.into(), vec![Hash::ORIGIN]))
            .add_block(DagBlock::new(2.into(), vec![1.into()]))
            .add_block(DagBlock::new(3.into(), vec![1.into()]))
            .add_block(DagBlock::new(4.into(), vec![2.into(), 3.into()]))
            .add_block(DagBlock::new(5.into(), vec![4.into()]))
            .add_block(DagBlock::new(6.into(), vec![1.into()]))
            .add_block(DagBlock::new(7.into(), vec![5.into(), 6.into()]))
            .add_block(DagBlock::new(8.into(), vec![1.into()]))
            .add_block(DagBlock::new(9.into(), vec![1.into()]))
            .add_block(DagBlock::new(10.into(), vec![7.into(), 8.into(), 9.into()]))
            .add_block(DagBlock::new(11.into(), vec![1.into()]))
            .add_block(DagBlock::new(12.into(), vec![11.into(), 10.into()]));

        // Assert intervals
        store.validate_intervals(Hash::ORIGIN).unwrap();

        // Assert genesis
        for i in 2u64..=12 {
            assert!(store.in_past_of(1, i));
        }

        // Assert some futures
        assert!(store.in_past_of(2, 4));
        assert!(store.in_past_of(2, 5));
        assert!(store.in_past_of(2, 7));
        assert!(store.in_past_of(5, 10));
        assert!(store.in_past_of(6, 10));
        assert!(store.in_past_of(10, 12));
        assert!(store.in_past_of(11, 12));

        // Assert some anticones
        assert!(store.are_anticone(2, 3));
        assert!(store.are_anticone(2, 6));
        assert!(store.are_anticone(3, 6));
        assert!(store.are_anticone(5, 6));
        assert!(store.are_anticone(3, 8));
        assert!(store.are_anticone(11, 2));
        assert!(store.are_anticone(11, 4));
        assert!(store.are_anticone(11, 6));
        assert!(store.are_anticone(11, 9));
    }

    #[test]
    fn test_forward_iterator() {
        // Arrange
        let mut store = MemoryReachabilityStore::new();

        // Act
        let root: Hash = 1.into();
        TreeBuilder::new(&mut store)
            .init_with_params(root, Interval::new(1, 15))
            .add_block(2.into(), root)
            .add_block(3.into(), 2.into())
            .add_block(4.into(), 2.into())
            .add_block(5.into(), 3.into())
            .add_block(6.into(), 5.into())
            .add_block(7.into(), 1.into())
            .add_block(8.into(), 6.into())
            .add_block(9.into(), 6.into())
            .add_block(10.into(), 6.into())
            .add_block(11.into(), 6.into());

        // Exclusive
        let iter = forward_chain_iterator(&store, 2.into(), 10.into(), false);

        // Assert
        let expected_hashes = [2u64, 3, 5, 6].map(Hash::from);
        assert!(expected_hashes
            .iter()
            .cloned()
            .eq(iter.map(|r| r.unwrap())));
        assert_eq!(
            store.get_height(2.into()).unwrap() + expected_hashes.len() as u64,
            store.get_height(10.into()).unwrap()
        );

        // Inclusive
        let iter = forward_chain_iterator(&store, 2.into(), 10.into(), true);

        // Assert
        let expected_hashes = [2u64, 3, 5, 6, 10].map(Hash::from);
        assert!(expected_hashes
            .iter()
            .cloned()
            .eq(iter.map(|r| r.unwrap())));

        // Compare backward to reversed forward
        let forward_iter = forward_chain_iterator(&store, 2.into(), 10.into(), true).map(|r| r.unwrap());
        let backward_iter: Result<Vec<Hash>> = backward_chain_iterator(&store, 10.into(), 2.into(), true).collect();
        assert!(forward_iter.eq(backward_iter.unwrap().iter().cloned().rev()))
    }

    #[test]
    fn test_iterator_boundaries() {
        // Arrange & Act
        let mut store = MemoryReachabilityStore::new();
        let root: Hash = 1.into();
        TreeBuilder::new(&mut store)
            .init_with_params(root, Interval::new(1, 5))
            .add_block(2.into(), root);

        // Asserts
        assert!([1u64, 2]
            .map(Hash::from)
            .iter()
            .cloned()
            .eq(forward_chain_iterator(&store, 1.into(), 2.into(), true).map(|r| r.unwrap())));

        assert!([1u64]
            .map(Hash::from)
            .iter()
            .cloned()
            .eq(forward_chain_iterator(&store, 1.into(), 2.into(), false).map(|r| r.unwrap())));

        assert!([2u64, 1]
            .map(Hash::from)
            .iter()
            .cloned()
            .eq(backward_chain_iterator(&store, 2.into(), root, true).map(|r| r.unwrap())));

        assert!([2u64]
            .map(Hash::from)
            .iter()
            .cloned()
            .eq(backward_chain_iterator(&store, 2.into(), root, false).map(|r| r.unwrap())));

        assert!(std::iter::once_with(|| root).eq(backward_chain_iterator(&store, root, root, true).map(|r| r.unwrap())));

        assert!(std::iter::empty::<Hash>().eq(backward_chain_iterator(&store, root, root, false).map(|r| r.unwrap())));
    }
}
