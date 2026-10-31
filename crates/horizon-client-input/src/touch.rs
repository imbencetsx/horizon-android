//! Multitouch ownership and layout, independent of Android and the renderer.
//! A finger owns its first target until release; crossing a button cannot fire it.
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Button {
    pub label: &'static str,
    pub action: &'static str,
    pub rect: [f32; 4],
}

/// Coordinates are physical pixels. Layout keeps targets at least 44 logical px
/// on a landscape phone; rendering and hit testing consume the same rectangles.
pub fn buttons(size: [f32; 2], density: f32) -> Vec<Button> {
    let d = density.max(1.0);
    let [w, h] = size.map(|v| v / d);
    let mut out = Vec::new();
    for (i, (label, action)) in [
        ("Jump", "key.jump"),
        ("Use", "key.use"),
        ("Mine", "key.attack"),
        ("Sneak", "key.sneak"),
        ("Run", "key.sprint"),
        ("Pick", "key.pickItem"),
    ]
    .into_iter()
    .enumerate()
    {
        let x = w - 12.0 - (3 - i % 3) as f32 * 56.0;
        let y = h - 12.0 - (2 - i / 3) as f32 * 52.0;
        out.push(Button {
            label,
            action,
            rect: [x * d, y * d, 52.0 * d, 48.0 * d],
        });
    }
    for (i, (label, action)) in [
        ("Pause", "back"),
        ("Chat", "key.chat"),
        ("Bag", "key.inventory"),
        ("Drop", "key.drop"),
        ("Swap", "key.swapOffhand"),
        ("<", "pad.hotbarPrevious"),
        (">", "pad.hotbarNext"),
    ]
    .into_iter()
    .enumerate()
    {
        out.push(Button {
            label,
            action,
            rect: [(12.0 + i as f32 * 52.0) * d, 12.0 * d, 48.0 * d, 44.0 * d],
        });
    }
    out
}

/// Menu tools expose secondary click, list scrolling and the system keyboard.
pub fn menu_buttons(size: [f32; 2], density: f32) -> Vec<Button> {
    let d = density.max(1.0);
    [
        ("Back", "back"),
        ("Right", "touch.right"),
        ("Up", "touch.up"),
        ("Down", "touch.down"),
        ("Keys", "touch.keyboard"),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (label, action))| Button {
        label,
        action,
        rect: [
            (12.0 + i as f32 * 52.0) * d,
            size[1] - 52.0 * d,
            48.0 * d,
            44.0 * d,
        ],
    })
    .collect()
}

#[derive(Clone, Debug, PartialEq)]
pub enum Output {
    Action(&'static str, bool),
    Look(f64, f64),
    Pointer([f32; 2]),
    Click(bool),
}
#[derive(Clone, Copy, Debug)]
enum Owner {
    Move([f32; 2]),
    Look([f32; 2]),
    Button(&'static str),
    Menu,
}
#[derive(Default, Debug)]
pub struct TouchControls {
    fingers: BTreeMap<u64, Owner>,
    movement: [bool; 4],
}
const MOVEMENT: [&str; 4] = ["key.forward", "key.back", "key.left", "key.right"];

impl TouchControls {
    /// A held finger remains input even when Android sends no move events.
    pub fn has_contacts(&self) -> bool {
        !self.fingers.is_empty()
    }

    pub fn active(&self, action: &str) -> bool {
        self.fingers
            .values()
            .any(|o| matches!(o, Owner::Button(a) if *a == action))
    }
    pub fn start(
        &mut self,
        id: u64,
        p: [f32; 2],
        size: [f32; 2],
        density: f32,
        playing: bool,
    ) -> Vec<Output> {
        if self.fingers.contains_key(&id) || (!playing && !self.fingers.is_empty()) {
            return Vec::new();
        }
        let mut out = Vec::new();
        let button = if playing {
            buttons(size, density)
        } else {
            menu_buttons(size, density)
        }
        .into_iter()
        .find(|b| {
            let [x, y, w, h] = b.rect;
            p[0] >= x && p[0] < x + w && p[1] >= y && p[1] < y + h
        });
        let owner = if let Some(b) = button {
            if !self.active(b.action) {
                out.push(Output::Action(b.action, true));
            }
            Owner::Button(b.action)
        } else if !playing {
            // Only one finger operates a menu's mouse pointer.
            if !self.fingers.is_empty() {
                return out;
            }
            out.extend([Output::Pointer(p), Output::Click(true)]);
            Owner::Menu
        } else if p[0] < size[0] * 0.36 && p[1] > size[1] * 0.40 {
            if self.fingers.values().any(|o| matches!(o, Owner::Move(_))) {
                return out;
            }
            Owner::Move(p)
        } else {
            if self.fingers.values().any(|o| matches!(o, Owner::Look(_))) {
                return out;
            }
            Owner::Look(p)
        };
        self.fingers.insert(id, owner);
        out
    }
    pub fn moved(&mut self, id: u64, p: [f32; 2], density: f32) -> Vec<Output> {
        let Some(owner) = self.fingers.get_mut(&id) else {
            return Vec::new();
        };
        match owner {
            Owner::Look(previous) => {
                let d = density.max(1.0);
                let out = Output::Look(
                    f64::from((p[0] - previous[0]) / d),
                    f64::from((p[1] - previous[1]) / d),
                );
                *previous = p;
                vec![out]
            }
            Owner::Move(origin) => {
                let dead = 12.0 * density.max(1.0);
                let dx = p[0] - origin[0];
                let dy = p[1] - origin[1];
                let next = [dy < -dead, dy > dead, dx < -dead, dx > dead];
                let out = (0..4)
                    .filter(|&i| next[i] != self.movement[i])
                    .map(|i| Output::Action(MOVEMENT[i], next[i]))
                    .collect();
                self.movement = next;
                out
            }
            Owner::Menu => vec![Output::Pointer(p)],
            Owner::Button(_) => Vec::new(),
        }
    }
    pub fn end(&mut self, id: u64) -> Vec<Output> {
        match self.fingers.remove(&id) {
            Some(Owner::Move(_)) => {
                let out = (0..4)
                    .filter(|&i| self.movement[i])
                    .map(|i| Output::Action(MOVEMENT[i], false))
                    .collect();
                self.movement = [false; 4];
                out
            }
            Some(Owner::Button(a)) if !self.active(a) => vec![Output::Action(a, false)],
            Some(Owner::Menu) => vec![Output::Click(false)],
            _ => Vec::new(),
        }
    }
    pub fn cancel(&mut self) -> Vec<Output> {
        let ids: Vec<_> = self.fingers.keys().copied().collect();
        ids.into_iter().flat_map(|id| self.end(id)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SIZE: [f32; 2] = [800.0, 400.0];
    #[test]
    fn move_look_and_mine_are_independent() {
        let mut c = TouchControls::default();
        assert!(!c.has_contacts());
        c.start(1, [80.0, 300.0], SIZE, 1.0, true);
        assert!(c.has_contacts());
        assert_eq!(
            c.moved(1, [80.0, 260.0], 1.0),
            vec![Output::Action("key.forward", true)]
        );
        c.start(2, [400.0, 200.0], SIZE, 1.0, true);
        assert_eq!(
            c.moved(2, [420.0, 190.0], 1.0),
            vec![Output::Look(20.0, -10.0)]
        );
        let b = buttons(SIZE, 1.0)
            .into_iter()
            .find(|b| b.action == "key.attack")
            .unwrap();
        assert_eq!(
            c.start(3, [b.rect[0] + 2.0, b.rect[1] + 2.0], SIZE, 1.0, true),
            vec![Output::Action("key.attack", true)]
        );
        let released = c.cancel();
        assert!(!c.has_contacts());
        assert!(released.contains(&Output::Action("key.forward", false)));
        assert!(released.contains(&Output::Action("key.attack", false)));
        assert!(c.cancel().is_empty());
    }
    #[test]
    fn second_button_finger_does_not_release_first() {
        let mut c = TouchControls::default();
        let b = buttons(SIZE, 1.0)[0];
        let p = [b.rect[0] + 1.0, b.rect[1] + 1.0];
        c.start(1, p, SIZE, 1.0, true);
        assert!(c.start(2, p, SIZE, 1.0, true).is_empty());
        assert!(c.end(1).is_empty());
        assert_eq!(c.end(2), vec![Output::Action(b.action, false)]);
    }
    #[test]
    fn menus_have_one_pointer_and_cancel_releases_it() {
        let mut c = TouchControls::default();
        assert_eq!(
            c.start(1, [20.0, 30.0], SIZE, 1.0, false),
            vec![Output::Pointer([20.0, 30.0]), Output::Click(true)]
        );
        assert!(c.start(2, [300.0, 30.0], SIZE, 1.0, false).is_empty());
        assert_eq!(c.cancel(), vec![Output::Click(false)]);
        assert!(c.moved(1, [50.0, 30.0], 1.0).is_empty());
    }
    #[test]
    fn finger_cannot_slide_onto_an_action() {
        let mut c = TouchControls::default();
        c.start(1, [400.0, 200.0], SIZE, 1.0, true);
        assert!(matches!(
            c.moved(1, [750.0, 300.0], 1.0).as_slice(),
            [Output::Look(..)]
        ));
    }
}

#[cfg(test)]
mod menu_tests {
    use super::*;
    #[test]
    fn menu_toolbar_cannot_change_button_during_a_drag() {
        let mut controls = TouchControls::default();
        let size = [800.0, 400.0];
        controls.start(1, [400.0, 150.0], size, 1.0, false);
        let right = menu_buttons(size, 1.0)[1].rect;
        assert!(
            controls
                .start(2, [right[0] + 1.0, right[1] + 1.0], size, 1.0, false)
                .is_empty()
        );
        assert_eq!(controls.end(1), vec![Output::Click(false)]);
    }
    #[test]
    fn density_scaled_button_centers_hit_the_drawn_action() {
        for density in [1.0, 2.0, 3.0] {
            let size = [800.0 * density, 400.0 * density];
            for playing in [false, true] {
                let layout = if playing {
                    buttons(size, density)
                } else {
                    menu_buttons(size, density)
                };
                for b in layout {
                    let [x, y, w, h] = b.rect;
                    assert!(w >= 44.0 * density && h >= 44.0 * density);
                    assert!(x >= 0.0 && y >= 0.0 && x + w <= size[0] && y + h <= size[1]);
                    let mut controls = TouchControls::default();
                    assert_eq!(
                        controls.start(1, [x + w * 0.5, y + h * 0.5], size, density, playing),
                        vec![Output::Action(b.action, true)]
                    );
                    assert_eq!(controls.end(1), vec![Output::Action(b.action, false)]);
                }
            }
        }
    }
    #[test]
    fn a_second_movement_finger_cannot_steal_or_release_movement() {
        let mut controls = TouchControls::default();
        let size = [800.0, 400.0];
        controls.start(1, [100.0, 300.0], size, 1.0, true);
        controls.moved(1, [60.0, 260.0], 1.0);
        assert!(
            controls
                .start(2, [100.0, 300.0], size, 1.0, true)
                .is_empty()
        );
        assert!(controls.end(2).is_empty());
        let out = controls.end(1);
        assert!(out.contains(&Output::Action("key.left", false)));
        assert!(out.contains(&Output::Action("key.forward", false)));
    }
}
