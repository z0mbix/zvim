use crate::terminal_tabs::TerminalTabs;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Divider {
    pub id: u64,
    pub axis: Axis,
    pub parent: Rect,
    pub rect: Rect,
}

enum Node {
    Pane(u64),
    Split {
        id: u64,
        axis: Axis,
        ratio: f32,
        first: Box<Node>,
        second: Box<Node>,
    },
}

pub struct TerminalLayout<T> {
    panes: Vec<(u64, TerminalTabs<T>)>,
    root: Option<Node>,
    focused: Option<u64>,
    next_id: u64,
}

impl<T> Default for TerminalLayout<T> {
    fn default() -> Self {
        Self {
            panes: Vec::new(),
            root: None,
            focused: None,
            next_id: 1,
        }
    }
}

impl<T> TerminalLayout<T> {
    fn id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
    pub fn focused_pane(&self) -> Option<u64> {
        self.focused
    }
    pub fn pane(&self, id: u64) -> Option<&TerminalTabs<T>> {
        self.panes
            .iter()
            .find(|(pane, _)| *pane == id)
            .map(|(_, tabs)| tabs)
    }
    pub fn focus_pane(&mut self, id: u64) -> bool {
        if self.pane(id).is_none() {
            return false;
        }
        self.focused = Some(id);
        true
    }
    pub fn focus_terminal(&mut self, id: u64) -> bool {
        let Some((pane, _)) = self
            .panes
            .iter()
            .find(|(_, tabs)| tabs.active_id() == Some(id))
        else {
            return false;
        };
        self.focused = Some(*pane);
        true
    }
    pub fn active(&self) -> Option<&T> {
        self.pane(self.focused?).and_then(|tabs| tabs.active())
    }
    pub fn active_id(&self) -> Option<u64> {
        self.pane(self.focused?).and_then(|tabs| tabs.active_id())
    }
    pub fn active_index(&self) -> usize {
        self.focused
            .and_then(|id| self.pane(id))
            .map_or(0, |tabs| tabs.active_index())
    }
    pub fn len(&self) -> usize {
        self.focused
            .and_then(|id| self.pane(id))
            .map_or(0, |tabs| tabs.len())
    }
    pub fn is_empty(&self) -> bool {
        self.panes.is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = &(u64, T)> {
        self.panes.iter().flat_map(|(_, tabs)| tabs.iter())
    }
    pub fn visible_ids(&self, zoom: bool) -> Vec<u64> {
        self.panes
            .iter()
            .filter(|(id, _)| !zoom || Some(*id) == self.focused)
            .filter_map(|(_, tabs)| tabs.active_id())
            .collect()
    }
    pub fn select(&mut self, index: usize) -> bool {
        self.panes
            .iter_mut()
            .find(|(id, _)| Some(*id) == self.focused)
            .is_some_and(|(_, tabs)| tabs.select(index))
    }
    pub fn adjacent(&self, next: bool) -> usize {
        self.focused
            .and_then(|id| self.pane(id))
            .map_or(0, |tabs| tabs.adjacent(next))
    }
    pub fn push(&mut self, value: T) -> u64 {
        let tab = self.id();
        if self.focused.is_none() {
            let pane = self.id();
            self.root = Some(Node::Pane(pane));
            self.focused = Some(pane);
            self.panes.push((pane, TerminalTabs::default()));
        }
        let (_, tabs) = self
            .panes
            .iter_mut()
            .find(|(id, _)| Some(*id) == self.focused)
            .unwrap();
        tabs.push_with_id(tab, value);
        tab
    }
    pub fn split(&mut self, pane: u64, axis: Axis, value: T) -> Option<u64> {
        self.pane(pane)?;
        let tab = self.id();
        let new_pane = self.id();
        let divider = self.id();
        let mut tabs = TerminalTabs::default();
        tabs.push_with_id(tab, value);
        self.root.as_mut()?.split(pane, new_pane, divider, axis);
        self.panes.push((new_pane, tabs));
        self.focused = Some(new_pane);
        Some(tab)
    }
    pub fn remove(&mut self, tab: u64) -> Option<T> {
        let index = self
            .panes
            .iter()
            .position(|(_, tabs)| tabs.iter().any(|(id, _)| *id == tab))?;
        let removed = self.panes[index].1.remove(tab)?;
        if self.panes[index].1.is_empty() {
            let pane = self.panes.remove(index).0;
            let neighbour = self.root.as_ref().and_then(|root| root.sibling(pane));
            self.root = self.root.take().and_then(|root| root.remove(pane));
            if self.focused == Some(pane) {
                self.focused = neighbour.or_else(|| self.root.as_ref().map(Node::first_pane));
            }
        }
        Some(removed)
    }
    pub fn layout(&self, width: f32, height: f32, zoom: bool) -> (Vec<(u64, Rect)>, Vec<Divider>) {
        let rect = Rect {
            x: 0.,
            y: 0.,
            width: width.max(0.),
            height: height.max(0.),
        };
        if zoom {
            return (
                self.focused.map(|id| vec![(id, rect)]).unwrap_or_default(),
                vec![],
            );
        }
        let mut panes = vec![];
        let mut dividers = vec![];
        if let Some(root) = &self.root {
            root.layout(rect, &mut panes, &mut dividers);
        }
        (panes, dividers)
    }
    pub fn resize(&mut self, divider: u64, ratio: f32) {
        if ratio.is_finite()
            && let Some(root) = &mut self.root
        {
            root.resize(divider, ratio.clamp(0., 1.));
        }
    }
    pub fn neighbour(&self, direction: Direction, width: f32, height: f32) -> Option<u64> {
        let (panes, _) = self.layout(width, height, false);
        let (_, current) = panes.iter().find(|(id, _)| Some(*id) == self.focused)?;
        let cx = current.x + current.width / 2.;
        let cy = current.y + current.height / 2.;
        panes
            .iter()
            .filter_map(|(id, rect)| {
                if Some(*id) == self.focused {
                    return None;
                }
                let x = rect.x + rect.width / 2.;
                let y = rect.y + rect.height / 2.;
                let (distance, cross, overlap) = match direction {
                    Direction::Left if rect.x + rect.width <= current.x + 1. => (
                        cx - x,
                        (cy - y).abs(),
                        rect.y < current.y + current.height && rect.y + rect.height > current.y,
                    ),
                    Direction::Right if rect.x >= current.x + current.width - 1. => (
                        x - cx,
                        (cy - y).abs(),
                        rect.y < current.y + current.height && rect.y + rect.height > current.y,
                    ),
                    Direction::Up if rect.y + rect.height <= current.y + 1. => (
                        cy - y,
                        (cx - x).abs(),
                        rect.x < current.x + current.width && rect.x + rect.width > current.x,
                    ),
                    Direction::Down if rect.y >= current.y + current.height - 1. => (
                        y - cy,
                        (cx - x).abs(),
                        rect.x < current.x + current.width && rect.x + rect.width > current.x,
                    ),
                    _ => return None,
                };
                overlap.then_some((*id, distance + cross))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }
}

impl Node {
    fn first_pane(&self) -> u64 {
        match self {
            Self::Pane(id) => *id,
            Self::Split { first, .. } => first.first_pane(),
        }
    }
    fn split(&mut self, pane: u64, new_pane: u64, id: u64, axis: Axis) {
        match self {
            Self::Pane(existing) if *existing == pane => {
                *self = Self::Split {
                    id,
                    axis,
                    ratio: 0.5,
                    first: Box::new(Self::Pane(pane)),
                    second: Box::new(Self::Pane(new_pane)),
                };
            }
            Self::Split { first, second, .. } => {
                first.split(pane, new_pane, id, axis);
                second.split(pane, new_pane, id, axis);
            }
            _ => {}
        }
    }
    fn sibling(&self, pane: u64) -> Option<u64> {
        match self {
            Self::Split { first, second, .. } => {
                if matches!(**first, Self::Pane(id) if id == pane) {
                    return Some(second.first_pane());
                }
                if matches!(**second, Self::Pane(id) if id == pane) {
                    return Some(first.first_pane());
                }
                first.sibling(pane).or_else(|| second.sibling(pane))
            }
            Self::Pane(_) => None,
        }
    }
    fn remove(self, pane: u64) -> Option<Self> {
        match self {
            Self::Pane(id) => (id != pane).then_some(Self::Pane(id)),
            Self::Split {
                id,
                axis,
                ratio,
                first,
                second,
            } => match (first.remove(pane), second.remove(pane)) {
                (Some(first), Some(second)) => Some(Self::Split {
                    id,
                    axis,
                    ratio,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (Some(node), None) | (None, Some(node)) => Some(node),
                (None, None) => None,
            },
        }
    }
    fn resize(&mut self, target: u64, value: f32) {
        if let Self::Split {
            id,
            ratio,
            first,
            second,
            ..
        } = self
        {
            if *id == target {
                *ratio = value;
            } else {
                first.resize(target, value);
                second.resize(target, value);
            }
        }
    }
    fn layout(&self, rect: Rect, panes: &mut Vec<(u64, Rect)>, dividers: &mut Vec<Divider>) {
        match self {
            Self::Pane(id) => panes.push((*id, rect)),
            Self::Split {
                id,
                axis,
                ratio,
                first,
                second,
            } => {
                let total = match axis {
                    Axis::Horizontal => rect.width,
                    Axis::Vertical => rect.height,
                };
                let gap = total.min(1.);
                let space = (total - gap).max(0.);
                let minimum = 80_f32.min(space / 2.);
                let extent = (space * ratio).clamp(minimum, space - minimum);
                let (a, b, divider) = match axis {
                    Axis::Horizontal => (
                        Rect {
                            width: extent,
                            ..rect
                        },
                        Rect {
                            x: rect.x + extent + gap,
                            width: space - extent,
                            ..rect
                        },
                        Rect {
                            x: rect.x + extent,
                            width: gap,
                            ..rect
                        },
                    ),
                    Axis::Vertical => (
                        Rect {
                            height: extent,
                            ..rect
                        },
                        Rect {
                            y: rect.y + extent + gap,
                            height: space - extent,
                            ..rect
                        },
                        Rect {
                            y: rect.y + extent,
                            height: gap,
                            ..rect
                        },
                    ),
                };
                dividers.push(Divider {
                    id: *id,
                    axis: *axis,
                    parent: rect,
                    rect: divider,
                });
                first.layout(a, panes, dividers);
                second.layout(b, panes, dividers);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn background_exit_preserves_focus_and_expands_nested_survivors() {
        let mut layout = TerminalLayout::default();
        let top_tab = layout.push("top");
        let top = layout.focused_pane().unwrap();
        let bottom_tab = layout.split(top, Axis::Vertical, "bottom left").unwrap();
        let bottom = layout.focused_pane().unwrap();
        let right_tab = layout
            .split(bottom, Axis::Horizontal, "bottom right")
            .unwrap();
        layout.focus_pane(top);
        layout.remove(right_tab);
        assert_eq!(layout.active_id(), Some(top_tab));
        let (panes, _) = layout.layout(800., 600., false);
        assert_eq!(
            panes.iter().find(|(id, _)| *id == bottom).unwrap().1.width,
            800.
        );
        layout.remove(bottom_tab);
        let (panes, dividers) = layout.layout(800., 600., false);
        assert_eq!(panes.len(), 1);
        assert_eq!(panes[0].1.width, 800.);
        assert_eq!(panes[0].1.height, 600.);
        assert!(dividers.is_empty());
        layout.remove(top_tab);
        assert!(layout.is_empty());
    }

    #[test]
    fn split_panes_own_independent_tabs_and_collapse_on_last_close() {
        let mut l = TerminalLayout::default();
        let first = l.push("one");
        let left = l.focused_pane().unwrap();
        let second = l.split(left, Axis::Horizontal, "two").unwrap();
        let right = l.focused_pane().unwrap();
        l.push("three");
        assert_eq!(l.len(), 2);
        assert_eq!(l.iter().count(), 3);
        l.focus_pane(left);
        assert_eq!(l.active(), Some(&"one"));
        assert_eq!(l.len(), 1);
        assert_eq!(l.neighbour(Direction::Right, 600., 300.), Some(right));
        l.remove(first);
        assert_eq!(l.layout(600., 300., false).0.len(), 1);
        assert_eq!(l.focused_pane(), Some(right));
        l.remove(second);
        assert_eq!(l.active(), Some(&"three"));
        l.remove(l.active_id().unwrap());
        assert!(l.is_empty());
    }
    #[test]
    fn nested_layout_resize_zoom_and_directional_focus() {
        let mut l = TerminalLayout::default();
        l.push(1);
        let left = l.focused_pane().unwrap();
        l.split(left, Axis::Horizontal, 2);
        let top = l.focused_pane().unwrap();
        l.split(top, Axis::Vertical, 3);
        let bottom = l.focused_pane().unwrap();
        assert_eq!(l.neighbour(Direction::Up, 600., 400.), Some(top));
        assert_eq!(l.neighbour(Direction::Left, 600., 400.), Some(left));
        l.focus_pane(top);
        assert_eq!(l.neighbour(Direction::Down, 600., 400.), Some(bottom));
        assert_eq!(l.neighbour(Direction::Right, 600., 400.), None);
        let (_, dividers) = l.layout(600., 400., false);
        l.resize(dividers[0].id, 0.25);
        assert_eq!(l.layout(600., 400., false).0[0].1.width, 599. * 0.25);
        assert_eq!(l.layout(600., 400., true).0.len(), 1);
        assert_eq!(l.visible_ids(true).len(), 1);
        assert_eq!(l.visible_ids(false).len(), 3);
        for (w, h) in [(0., 0.), (20., 30.), (600., 400.)] {
            let (panes, _) = l.layout(w, h, false);
            assert!(panes.iter().all(|(_, r)| r.width >= 0. && r.height >= 0.));
        }
    }
    #[test]
    fn close_nested_pane_focuses_its_sibling_and_preserves_other_tabs() {
        let mut l = TerminalLayout::default();
        l.push("left");
        let left = l.focused_pane().unwrap();
        l.split(left, Axis::Horizontal, "top right");
        let top = l.focused_pane().unwrap();
        let bottom_tab = l.split(top, Axis::Vertical, "bottom right").unwrap();
        l.remove(bottom_tab);
        assert_eq!(l.focused_pane(), Some(top));
        assert_eq!(l.iter().count(), 2);
        let (panes, dividers) = l.layout(640., 400., false);
        assert_eq!(panes.len(), 2);
        assert_eq!(dividers.len(), 1);
        l.focus_pane(left);
        let new_tab = l.push("left second tab");
        assert_eq!(l.visible_ids(false).len(), 2);
        l.select(0);
        assert!(!l.visible_ids(false).contains(&new_tab));
        assert!(!l.focus_terminal(new_tab));
    }
}
