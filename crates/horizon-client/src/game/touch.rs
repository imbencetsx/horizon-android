//! Touch controls feed the same action mappings and menu pointer as controllers.
use super::*;
use horizon_client_input::touch::{self, Output};
use winit::event::{Touch, TouchPhase};

impl Game<'_> {
    pub(super) fn touch_playing(&self) -> bool {
        self.screen == Screen::Playing
            && self.title.is_none()
            && !self.dialog_open()
            && self.server_packs.prompt.is_none()
            && self.link_prompt.is_none()
            && self.code_of_conduct.is_none()
    }
    pub(super) fn touch_event(&mut self, event_loop: &ActiveEventLoop, event: Touch) -> Result<()> {
        let Some(window) = self.window.as_ref() else {
            return Ok(());
        };
        let size = window.inner_size();
        let density = window.scale_factor() as f32;
        let point = [event.location.x as f32, event.location.y as f32];
        let playing = self.touch_playing();
        let gui_scale = HudBatch::new([size.width, size.height]).scale;
        let outputs = match event.phase {
            TouchPhase::Started => self.touch.start_with_gui_scale(
                event.id,
                point,
                [size.width as f32, size.height as f32],
                density,
                gui_scale,
                playing,
            ),
            TouchPhase::Moved => self.touch.moved(event.id, point, density),
            TouchPhase::Ended => self.touch.end(event.id),
            TouchPhase::Cancelled => self.touch.cancel_finger(event.id),
        };
        self.last_input = Instant::now();
        self.touch_outputs(event_loop, outputs)?;
        if playing != self.touch_playing() {
            let outputs = self.touch.cancel();
            self.touch_outputs(event_loop, outputs)?;
        }
        Ok(())
    }
    pub(super) fn touch_outputs(
        &mut self,
        event_loop: &ActiveEventLoop,
        outputs: Vec<Output>,
    ) -> Result<()> {
        for output in outputs {
            match output {
                Output::Action("back", true) => self.pad_back()?,
                Output::Action("back", false) => {}
                Output::Hotbar(slot) => {
                    let key = match slot {
                        0 => KeyCode::Digit1,
                        1 => KeyCode::Digit2,
                        2 => KeyCode::Digit3,
                        3 => KeyCode::Digit4,
                        4 => KeyCode::Digit5,
                        5 => KeyCode::Digit6,
                        6 => KeyCode::Digit7,
                        7 => KeyCode::Digit8,
                        8 => KeyCode::Digit9,
                        _ => continue,
                    };
                    self.key_pressed(key)?;
                }
                Output::Tap => {
                    // A held block item uses the aimed-at block; other short
                    // taps swing at the current target. Mine is held separately.
                    let button = if self.pick().is_some() || self.held().is_some() {
                        MouseButton::Right
                    } else {
                        MouseButton::Left
                    };
                    self.play_mouse(button, true)?;
                    self.play_mouse(button, false)?;
                }
                Output::Action("touch.right", down) => {
                    if down {
                        self.touch_right = !self.touch_right;
                    }
                }
                Output::Action("touch.keyboard", down) => {
                    #[cfg(target_os = "android")]
                    if down {
                        crate::android::app().show_soft_input(false);
                    }
                    let _ = down;
                }
                Output::Action(action @ ("touch.up" | "touch.down"), down) => {
                    if down && let Some(id) = self.window.as_ref().map(|w| w.id()) {
                        let (width, height) = self.gui_size();
                        // A first scroll has no list pointer yet. Aim at the
                        // viewport rather than the bottom toolbar.
                        if self.cursor.1 < height * 0.2 || self.cursor.1 > height * 0.8 {
                            let scale = self.renderer.as_ref().map_or(1.0, |r| {
                                HudBatch::new([r.config.width, r.config.height]).scale
                            });
                            self.window_event(
                                event_loop,
                                id,
                                WindowEvent::CursorMoved {
                                    device_id: DeviceId::dummy(),
                                    position: winit::dpi::PhysicalPosition::new(
                                        f64::from(width * 0.5 * scale),
                                        f64::from(height * 0.5 * scale),
                                    ),
                                },
                            );
                        }
                        self.window_event(
                            event_loop,
                            id,
                            WindowEvent::MouseWheel {
                                device_id: DeviceId::dummy(),
                                delta: MouseScrollDelta::LineDelta(
                                    0.0,
                                    if action == "touch.up" { 3.0 } else { -3.0 },
                                ),
                                phase: winit::event::TouchPhase::Moved,
                            },
                        );
                    }
                }
                Output::Action(action, down) => {
                    // Feed the game's own mapping and toggle settings.
                    if action == "key.sneak" {
                        self.mapped_key(KeyCode::ShiftLeft, down, false)?;
                    } else {
                        self.pad_action(action, down)?;
                    }
                }
                Output::Look(x, y) if self.touch_playing() => {
                    self.mouse.on_move(0.0, 0.0, x, y, true, true);
                }
                Output::Look(..) => {}
                Output::Pointer([x, y]) => {
                    if let Some(id) = self.window.as_ref().map(|w| w.id()) {
                        self.window_event(
                            event_loop,
                            id,
                            WindowEvent::CursorMoved {
                                device_id: DeviceId::dummy(),
                                position: winit::dpi::PhysicalPosition::new(
                                    f64::from(x),
                                    f64::from(y),
                                ),
                            },
                        );
                    }
                }
                Output::Click(down) => {
                    if let Some(id) = self.window.as_ref().map(|w| w.id()) {
                        self.window_event(
                            event_loop,
                            id,
                            WindowEvent::MouseInput {
                                device_id: DeviceId::dummy(),
                                button: if self.touch_right {
                                    MouseButton::Right
                                } else {
                                    MouseButton::Left
                                },
                                state: if down {
                                    ElementState::Pressed
                                } else {
                                    ElementState::Released
                                },
                            },
                        );
                    }
                }
            }
        }
        Ok(())
    }
    pub(super) fn draw_touch(&self, batch: &mut HudBatch) {
        if !cfg!(target_os = "android") {
            return;
        }
        let Some(window) = &self.window else {
            return;
        };
        let size = window.inner_size();
        let density = window.scale_factor() as f32;
        let scale = batch.scale;
        let playing = self.touch_playing();
        let extent = [size.width as f32, size.height as f32];
        let buttons = if playing {
            touch::buttons(extent, density)
        } else {
            touch::menu_buttons(extent, density)
        };
        for button in buttons {
            let [x, y, w, h] = button.rect.map(|v| v / scale);
            let color = if self.touch.active(button.action)
                || (button.action == "touch.right" && self.touch_right)
            {
                rgba(100, 110, 100, 210)
            } else {
                rgba(25, 25, 25, 150)
            };
            batch.fill(&self.gui.atlas, x, y, w, h, color);
            let tw = HudBatch::text_width(&self.gui.atlas, button.label);
            batch.text(
                &self.gui.atlas,
                button.label,
                x + (w - tw) * 0.5,
                y + (h - 8.0) * 0.5,
                rgba(255, 255, 255, 255),
                true,
            );
        }
        if !playing {
            return;
        }
        let d = density / scale;
        let y = batch.height - 100.0 * d;
        batch.fill(
            &self.gui.atlas,
            24.0 * d,
            y - 60.0 * d,
            140.0 * d,
            120.0 * d,
            rgba(25, 25, 25, 100),
        );
        batch.text(
            &self.gui.atlas,
            "Move",
            70.0 * d,
            y,
            rgba(255, 255, 255, 210),
            true,
        );
    }
}
