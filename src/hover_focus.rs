//! Pointer-driven focus policy, independent of native rendering and input delivery.
use crate::terminal_layout::Rect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Editor,
    Terminal(u64),
}
#[derive(Default)]
pub struct Pointer {
    position: Option<(f32, f32)>,
}
impl Pointer {
    pub fn reset(&mut self, position: (f32, f32)) {
        self.position = Some(position);
    }
    pub fn moved(&mut self, position: (f32, f32), permitted: bool) -> bool {
        let changed = self.position != Some(position);
        self.position = Some(position);
        changed && permitted
    }
}
pub fn target(
    position: (f32, f32),
    editor: Option<Rect>,
    terminals: &[(u64, Rect)],
    excluded: &[Rect],
) -> Option<Target> {
    if excluded.iter().any(|rect| contains(*rect, position)) {
        return None;
    }
    terminals
        .iter()
        .find(|(_, rect)| contains(*rect, position))
        .map(|(pane, _)| Target::Terminal(*pane))
        .or_else(|| {
            editor
                .filter(|rect| contains(*rect, position))
                .map(|_| Target::Editor)
        })
}
fn contains(rect: Rect, (x, y): (f32, f32)) -> bool {
    x >= rect.x && y >= rect.y && x < rect.x + rect.width && y < rect.y + rect.height
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keyboard_focus_survives_stationary_events_and_suppressed_movement() {
        let mut pointer = Pointer::default();
        pointer.reset((10., 10.));
        assert!(!pointer.moved((10., 10.), true));
        assert!(pointer.moved((11., 10.), true));
        // Inactive windows, disabled settings, dialogs and drags consume movement.
        assert!(!pointer.moved((100., 100.), false));
        assert!(!pointer.moved((100., 100.), true));
        assert!(pointer.moved((101., 100.), true));
        pointer.reset((200., 200.));
        assert!(!pointer.moved((200., 200.), true));
    }
    #[test]
    fn content_only_hit_testing_respects_tabs_dividers_and_hidden_panes() {
        let editor = Rect {
            x: 0.,
            y: 0.,
            width: 800.,
            height: 300.,
        };
        let panes = [
            (
                1,
                Rect {
                    x: 0.,
                    y: 325.,
                    width: 400.,
                    height: 275.,
                },
            ),
            (
                2,
                Rect {
                    x: 401.,
                    y: 325.,
                    width: 399.,
                    height: 275.,
                },
            ),
        ];
        let excluded = [
            Rect {
                x: 397.,
                y: 301.,
                width: 7.,
                height: 299.,
            },
            Rect {
                x: 0.,
                y: 298.,
                width: 800.,
                height: 7.,
            },
        ];
        assert_eq!(
            target((50., 50.), Some(editor), &panes, &excluded),
            Some(Target::Editor)
        );
        assert_eq!(
            target((50., 350.), Some(editor), &panes, &excluded),
            Some(Target::Terminal(1))
        );
        assert_eq!(
            target((450., 350.), Some(editor), &panes, &excluded),
            Some(Target::Terminal(2))
        );
        for point in [(50., 310.), (399., 400.), (50., 299.), (801., 400.)] {
            assert_eq!(target(point, Some(editor), &panes, &excluded), None);
        }
        assert_eq!(target((450., 350.), None, &panes[..1], &excluded), None);
        assert_eq!(target((50., 350.), Some(editor), &[], &[]), None);
    }
}
