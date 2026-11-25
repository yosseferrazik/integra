//! Mosaic layout: a binary split tree with draggable dividers.

use ratatui::layout::Rect;

pub const MIN_RATIO: u16 = 10;
pub const MAX_RATIO: u16 = 90;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// Side by side (vertical divider).
    X,
    /// Stacked (horizontal divider).
    Y,
}

#[derive(Clone, Debug)]
pub enum Node {
    Leaf(usize),
    Split {
        axis: Axis,
        ratio: u16,
        a: Box<Node>,
        b: Box<Node>,
    },
}

impl Node {
    pub fn leaf(id: usize) -> Self {
        Self::Leaf(id)
    }

    pub fn split(axis: Axis, ratio: u16, a: Node, b: Node) -> Self {
        Self::Split {
            axis,
            ratio,
            a: Box::new(a),
            b: Box::new(b),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Divider {
    pub rect: Rect,
    pub parent: Rect,
    pub axis: Axis,
    pub path: Vec<bool>,
}

/// Computes every pane rectangle and divider hit area inside `area`.
pub fn compute(root: &Node, area: Rect) -> (Vec<(usize, Rect)>, Vec<Divider>) {
    let mut rects = Vec::new();
    let mut dividers = Vec::new();
    walk(root, area, &mut Vec::new(), &mut rects, &mut dividers);
    (rects, dividers)
}

fn walk(
    node: &Node,
    area: Rect,
    path: &mut Vec<bool>,
    rects: &mut Vec<(usize, Rect)>,
    dividers: &mut Vec<Divider>,
) {
    match node {
        Node::Leaf(id) => rects.push((*id, area)),
        Node::Split { axis, ratio, a, b } => {
            let (first, gap, second) = split_sizes(area, *axis, *ratio);
            let (a_rect, gap_rect, b_rect) = match axis {
                Axis::X => (
                    Rect {
                        width: first,
                        ..area
                    },
                    Rect {
                        x: area.x + first,
                        y: area.y,
                        width: gap,
                        height: area.height,
                    },
                    Rect {
                        x: area.x + first + gap,
                        y: area.y,
                        width: second,
                        height: area.height,
                    },
                ),
                Axis::Y => (
                    Rect {
                        height: first,
                        ..area
                    },
                    Rect {
                        x: area.x,
                        y: area.y + first,
                        width: area.width,
                        height: gap,
                    },
                    Rect {
                        x: area.x,
                        y: area.y + first + gap,
                        width: area.width,
                        height: second,
                    },
                ),
            };
            if gap > 0 {
                dividers.push(Divider {
                    rect: gap_rect,
                    parent: area,
                    axis: *axis,
                    path: path.clone(),
                });
            }
            path.push(false);
            walk(a, a_rect, path, rects, dividers);
            path.pop();
            path.push(true);
            walk(b, b_rect, path, rects, dividers);
            path.pop();
        }
    }
}

/// Splits a length into (first, divider, second), leaving room for both panes.
fn split_sizes(area: Rect, axis: Axis, ratio: u16) -> (u16, u16, u16) {
    let total = match axis {
        Axis::X => area.width,
        Axis::Y => area.height,
    };
    if total < 3 {
        return (total, 0, 0);
    }
    let ratio = ratio.clamp(MIN_RATIO, MAX_RATIO) as u32;
    let first = ((total as u32 * ratio / 100) as u16).clamp(1, total - 2);
    (first, 1, total - first - 1)
}

/// Path of left/right turns from the root to `target`, if present.
pub fn path_to(node: &Node, target: usize) -> Option<Vec<bool>> {
    fn find(node: &Node, target: usize, path: &mut Vec<bool>) -> bool {
        match node {
            Node::Leaf(id) => *id == target,
            Node::Split { a, b, .. } => {
                path.push(false);
                if find(a, target, path) {
                    return true;
                }
                path.pop();
                path.push(true);
                if find(b, target, path) {
                    return true;
                }
                path.pop();
                false
            }
        }
    }
    let mut path = Vec::new();
    find(node, target, &mut path).then_some(path)
}

pub fn node_at_mut<'a>(node: &'a mut Node, path: &[bool]) -> Option<&'a mut Node> {
    let mut current = node;
    for &right in path {
        current = match current {
            Node::Split { a, b, .. } => {
                if right {
                    b
                } else {
                    a
                }
            }
            Node::Leaf(_) => return None,
        };
    }
    Some(current)
}

/// Replaces the leaf `target` with a split holding it and `new_id`.
pub fn split_leaf(root: &mut Node, target: usize, new_id: usize, axis: Axis) -> bool {
    let Some(path) = path_to(root, target) else {
        return false;
    };
    let Some(node) = node_at_mut(root, &path) else {
        return false;
    };
    *node = Node::split(axis, 50, Node::leaf(target), Node::leaf(new_id));
    true
}

/// Removes a leaf and collapses its parent. Returns `None` when it was the root.
pub fn remove_leaf(node: &Node, target: usize) -> Option<Node> {
    match node {
        Node::Leaf(id) => (*id != target).then_some(Node::Leaf(*id)),
        Node::Split { axis, ratio, a, b } => {
            match (remove_leaf(a, target), remove_leaf(b, target)) {
                (Some(a), Some(b)) => Some(Node::Split {
                    axis: *axis,
                    ratio: *ratio,
                    a: Box::new(a),
                    b: Box::new(b),
                }),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            }
        }
    }
}

pub fn set_ratio(root: &mut Node, path: &[bool], ratio: u16) {
    if let Some(Node::Split { ratio: current, .. }) = node_at_mut(root, path) {
        *current = ratio.clamp(MIN_RATIO, MAX_RATIO);
    }
}

/// Adjusts the nearest ancestor split of `target` along `axis`.
pub fn resize(root: &mut Node, target: usize, axis: Axis, delta: i16) -> bool {
    let Some(path) = path_to(root, target) else {
        return false;
    };
    let mut deepest: Option<Vec<bool>> = None;
    {
        let mut node: &Node = root;
        for (depth, &right) in path.iter().enumerate() {
            let Node::Split {
                axis: split_axis,
                a,
                b,
                ..
            } = node
            else {
                break;
            };
            if *split_axis == axis {
                deepest = Some(path[..depth].to_vec());
            }
            node = if right { &**b } else { &**a };
        }
    }
    let Some(path) = deepest else {
        return false;
    };
    let Some(Node::Split { ratio, .. }) = node_at_mut(root, &path) else {
        return false;
    };
    *ratio = (*ratio as i16 + delta).clamp(MIN_RATIO as i16, MAX_RATIO as i16) as u16;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_layout_has_no_overlap() {
        let root = Node::split(
            Axis::X,
            30,
            Node::leaf(1),
            Node::split(Axis::Y, 50, Node::leaf(2), Node::leaf(3)),
        );
        let (rects, dividers) = compute(&root, Rect::new(0, 0, 100, 40));
        assert_eq!(rects.len(), 3);
        assert_eq!(dividers.len(), 2);
        let editor = rects.iter().find(|(id, _)| *id == 1).unwrap().1;
        assert_eq!(editor.width, 30);
        assert_eq!(editor.height, 40);
    }

    #[test]
    fn resize_targets_deepest_matching_split() {
        let mut root = Node::split(
            Axis::X,
            30,
            Node::leaf(1),
            Node::split(Axis::X, 40, Node::leaf(2), Node::leaf(3)),
        );
        assert!(resize(&mut root, 3, Axis::X, 10));
        match &root {
            Node::Split { ratio, b, .. } => {
                assert_eq!(*ratio, 30);
                if let Node::Split { ratio, .. } = &**b {
                    assert_eq!(*ratio, 50);
                } else {
                    panic!("expected nested split");
                }
            }
            _ => panic!("expected split"),
        }
    }

    #[test]
    fn removing_a_leaf_collapses_its_parent() {
        let root = Node::split(Axis::X, 50, Node::leaf(1), Node::leaf(2));
        let root = remove_leaf(&root, 2).expect("root should survive");
        assert!(matches!(root, Node::Leaf(1)));
    }
}
