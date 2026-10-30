use wafer_core::BlockPos;

use super::node::Node;
use super::path::Path;

fn path() -> Path {
    Path::new((0..3).map(|x| Node::new(x, 0, 0)).collect(), BlockPos::new(2, 0, 0), true)
}

#[test]
fn cloned_paths_are_the_same_object() {
    let path = path();
    let alias = path.clone();
    assert!(!alias.is_done());
    path.advance();
    path.advance();
    assert_eq!(alias.get_next_node_index(), 2);
    path.advance();
    assert!(alias.is_done());
}

#[test]
fn changes_to_the_nodes_are_seen_through_every_handle() {
    let path = path();
    let alias = path.clone();
    alias.replace_node(1, Node::new(1, 1, 0));
    assert_eq!(path.get_node(1).y, 1);
    alias.truncate_nodes(1);
    assert_eq!(path.get_node_count(), 1);
    path.set_next_node_index(1);
    assert!(alias.is_done());
}

#[test]
fn same_as_compares_the_nodes() {
    let path = path();
    assert!(path.same_as(Some(&path.clone())));
    assert!(path.same_as(Some(&self::path())));
    assert!(!path.same_as(None));
    let shorter = Path::new(vec![Node::new(0, 0, 0)], BlockPos::new(0, 0, 0), true);
    assert!(!path.same_as(Some(&shorter)));
}
