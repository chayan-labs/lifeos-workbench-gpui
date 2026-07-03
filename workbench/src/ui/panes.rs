//! Zellij-model tiling layout: a binary split tree per tab, one focused pane
//! per tab. Pure data - no geometry, no rendering. Ported from the legacy
//! `layout.rs`, dropping `rects()` (that computed ratatui `Rect`s for the cell
//! grid; gpui tiles the tree with nested resizable panels instead, so screen
//! rectangles are the renderer's job, not this module's).
//!
//! Immutability convention: every operation returns a new tree.

pub type PaneId = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitDir {
    Horizontal, // children side by side
    Vertical,   // children stacked
}

#[derive(Clone, Debug, PartialEq)]
pub enum LayoutNode {
    Leaf(PaneId),
    Split {
        dir: SplitDir,
        first: Box<LayoutNode>,
        second: Box<LayoutNode>,
    },
}

impl LayoutNode {
    /// Split the target leaf in two; the new pane takes the second half.
    /// Returns the new tree, unchanged if `target` is not present.
    pub fn split(&self, target: PaneId, dir: SplitDir, new_pane: PaneId) -> LayoutNode {
        match self {
            LayoutNode::Leaf(id) if *id == target => LayoutNode::Split {
                dir,
                first: Box::new(LayoutNode::Leaf(target)),
                second: Box::new(LayoutNode::Leaf(new_pane)),
            },
            LayoutNode::Leaf(_) => self.clone(),
            LayoutNode::Split {
                dir: d,
                first,
                second,
            } => LayoutNode::Split {
                dir: *d,
                first: Box::new(first.split(target, dir, new_pane)),
                second: Box::new(second.split(target, dir, new_pane)),
            },
        }
    }

    /// Remove a pane; its sibling absorbs the space. `None` when the tree
    /// becomes empty (last pane closed).
    pub fn close(&self, target: PaneId) -> Option<LayoutNode> {
        match self {
            LayoutNode::Leaf(id) => (*id != target).then(|| self.clone()),
            LayoutNode::Split { dir, first, second } => {
                match (first.close(target), second.close(target)) {
                    (Some(f), Some(s)) => Some(LayoutNode::Split {
                        dir: *dir,
                        first: Box::new(f),
                        second: Box::new(s),
                    }),
                    (None, Some(s)) => Some(s),
                    (Some(f), None) => Some(f),
                    (None, None) => None,
                }
            }
        }
    }

    /// Pane ids in in-order (visual reading order).
    pub fn panes(&self) -> Vec<PaneId> {
        match self {
            LayoutNode::Leaf(id) => vec![*id],
            LayoutNode::Split { first, second, .. } => {
                let mut v = first.panes();
                v.extend(second.panes());
                v
            }
        }
    }
}

/// One tab: a layout tree plus its focused pane.
#[derive(Clone, Debug)]
pub struct Tab {
    pub root: LayoutNode,
    pub focused: PaneId,
}

/// The whole shell layout: tabs, an active tab, and a pane-id allocator.
#[derive(Clone, Debug)]
pub struct Layout {
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
    next_pane: PaneId,
}

impl Layout {
    pub fn new() -> Self {
        Self {
            tabs: vec![Tab {
                root: LayoutNode::Leaf(0),
                focused: 0,
            }],
            active_tab: 0,
            next_pane: 1,
        }
    }

    pub fn tab(&self) -> &Tab {
        &self.tabs[self.active_tab]
    }

    /// Split the focused pane; focus moves to the new pane. Returns the new
    /// layout and the id of the pane just created.
    pub fn split_focused(&self, dir: SplitDir) -> (Layout, PaneId) {
        let new_pane = self.next_pane;
        let tab = self.tab();
        let new_tab = Tab {
            root: tab.root.split(tab.focused, dir, new_pane),
            focused: new_pane,
        };
        (self.with_tab(new_tab, self.next_pane + 1), new_pane)
    }

    /// Close the focused pane; focus falls to the nearest remaining pane.
    /// Closing the last pane of the last tab returns `None` (quit signal).
    pub fn close_focused(&self) -> Option<Layout> {
        self.close_pane(self.tab().focused)
    }

    /// Close a specific pane of the active tab (mouse ×, shell `exit`).
    pub fn close_pane(&self, pane: PaneId) -> Option<Layout> {
        let tab = self.tab();
        match tab.root.close(pane) {
            Some(root) => {
                let panes = root.panes();
                let focused = if panes.contains(&tab.focused) {
                    tab.focused
                } else {
                    *panes.last().expect("non-empty tree has panes")
                };
                Some(self.with_tab(Tab { root, focused }, self.next_pane))
            }
            None if self.tabs.len() > 1 => {
                let mut tabs = self.tabs.clone();
                tabs.remove(self.active_tab);
                let active_tab = self.active_tab.min(tabs.len() - 1);
                Some(Layout {
                    tabs,
                    active_tab,
                    next_pane: self.next_pane,
                })
            }
            None => None,
        }
    }

    /// Cycle focus through panes of the active tab in reading order.
    pub fn focus_next(&self) -> Layout {
        self.refocus(1)
    }

    pub fn focus_prev(&self) -> Layout {
        self.refocus(-1)
    }

    fn refocus(&self, delta: isize) -> Layout {
        let tab = self.tab();
        let panes = tab.root.panes();
        let pos = panes.iter().position(|p| *p == tab.focused).unwrap_or(0);
        let n = panes.len() as isize;
        let focused = panes[(((pos as isize + delta) % n + n) % n) as usize];
        self.with_tab(
            Tab {
                root: tab.root.clone(),
                focused,
            },
            self.next_pane,
        )
    }

    /// Open a fresh tab with a single new pane and switch to it.
    pub fn new_tab(&self) -> (Layout, PaneId) {
        let pane = self.next_pane;
        let mut tabs = self.tabs.clone();
        tabs.push(Tab {
            root: LayoutNode::Leaf(pane),
            focused: pane,
        });
        (
            Layout {
                active_tab: tabs.len() - 1,
                tabs,
                next_pane: pane + 1,
            },
            pane,
        )
    }

    /// Focus a specific pane of the active tab (mouse click). Unchanged if the
    /// pane is not in this tab.
    pub fn focus_pane(&self, pane: PaneId) -> Layout {
        let tab = self.tab();
        if !tab.root.panes().contains(&pane) {
            return self.clone();
        }
        self.with_tab(
            Tab {
                root: tab.root.clone(),
                focused: pane,
            },
            self.next_pane,
        )
    }

    pub fn next_tab(&self) -> Layout {
        Layout {
            active_tab: (self.active_tab + 1) % self.tabs.len(),
            ..self.clone()
        }
    }

    /// Jump straight to a tab (tab-bar click); out-of-range clamps.
    pub fn switch_tab(&self, index: usize) -> Layout {
        Layout {
            active_tab: index.min(self.tabs.len() - 1),
            ..self.clone()
        }
    }

    fn with_tab(&self, tab: Tab, next_pane: PaneId) -> Layout {
        let mut tabs = self.tabs.clone();
        tabs[self.active_tab] = tab;
        Layout {
            tabs,
            active_tab: self.active_tab,
            next_pane,
        }
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_focused_creates_and_focuses_a_new_pane() {
        let layout = Layout::new();
        let (layout, new_pane) = layout.split_focused(SplitDir::Horizontal);
        assert_eq!(layout.tab().root.panes(), vec![0, new_pane]);
        assert_eq!(layout.tab().focused, new_pane);
    }

    #[test]
    fn close_focused_returns_space_to_sibling_and_refocuses() {
        let (layout, _) = Layout::new().split_focused(SplitDir::Vertical);
        let layout = layout.close_focused().expect("one pane remains");
        assert_eq!(layout.tab().root.panes(), vec![0]);
        assert_eq!(layout.tab().focused, 0);
    }

    #[test]
    fn closing_the_last_pane_of_the_last_tab_signals_quit() {
        assert!(Layout::new().close_focused().is_none());
    }

    #[test]
    fn focus_cycles_forward_and_backward() {
        let (layout, _) = Layout::new().split_focused(SplitDir::Horizontal);
        let (layout, third) = layout.split_focused(SplitDir::Vertical);
        assert_eq!(layout.tab().focused, third);
        let layout = layout.focus_next();
        assert_eq!(layout.tab().focused, 0);
        let layout = layout.focus_prev();
        assert_eq!(layout.tab().focused, third);
    }

    #[test]
    fn focus_pane_targets_a_specific_pane_and_ignores_strangers() {
        let (layout, _) = Layout::new().split_focused(SplitDir::Horizontal);
        let layout = layout.focus_pane(0);
        assert_eq!(layout.tab().focused, 0);
        let layout = layout.focus_pane(999);
        assert_eq!(layout.tab().focused, 0, "unknown pane leaves focus alone");
    }

    #[test]
    fn tabs_open_switch_and_close() {
        let (layout, pane) = Layout::new().new_tab();
        assert_eq!(layout.tabs.len(), 2);
        assert_eq!(layout.tab().focused, pane);
        let layout = layout.close_focused().expect("falls back to first tab");
        assert_eq!(layout.tabs.len(), 1);
        assert_eq!(layout.tab().focused, 0);
    }
}
