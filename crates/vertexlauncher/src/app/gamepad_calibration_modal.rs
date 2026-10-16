use config::GamepadCalibration;

use crate::app::gamepad::{GamepadDeviceIdentity, GamepadStickSample};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CalibrationStep {
    Neutral,
    Up,
    Down,
    Left,
    Right,
    Review,
}

#[derive(Debug, Clone, Default)]
pub struct GamepadCalibrationState {
    pub device: Option<GamepadDeviceIdentity>,
    step: Option<CalibrationStep>,
    neutral: Option<GamepadStickSample>,
    up: Option<GamepadStickSample>,
    down: Option<GamepadStickSample>,
    left: Option<GamepadStickSample>,
    right: Option<GamepadStickSample>,
    error: Option<String>,
}

impl GamepadCalibrationState {
    pub fn start(&mut self, device: GamepadDeviceIdentity) {
        self.device = Some(device);
        self.step = Some(CalibrationStep::Neutral);
        self.neutral = None;
        self.up = None;
        self.down = None;
        self.left = None;
        self.right = None;
        self.error = None;
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn device_key(&self) -> Option<&str> {
        self.device.as_ref().map(|device| device.key.as_str())
    }

    /// What the user should do now, or `None` when no calibration is running.
    pub fn instructions(&self) -> Option<&'static str> {
        self.step.map(step_description)
    }

    /// The label of the main button for the current step.
    pub fn primary_label(&self) -> &'static str {
        match self.step {
            Some(CalibrationStep::Neutral) => "Use current neutral",
            Some(CalibrationStep::Review) => "Save calibration",
            _ => "Capture current position",
        }
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The main button was pressed: capture the current stick position for this step, or, on the last step, finish.
    pub fn primary(&mut self, live_sample: Option<GamepadStickSample>) -> ModalAction {
        let (Some(device), Some(step)) = (self.device.clone(), self.step) else {
            return ModalAction::None;
        };
        match step {
            CalibrationStep::Review => match build_calibration(self) {
                Some(calibration) => ModalAction::Save {
                    device_key: device.key,
                    calibration,
                },
                None => {
                    self.error = Some(
                        "Calibration is incomplete or the captured values are inconsistent."
                            .to_owned(),
                    );
                    ModalAction::None
                }
            },
            _ => match live_sample {
                Some(sample) => {
                    capture_step(self, step, sample);
                    ModalAction::None
                }
                None => {
                    self.error =
                        Some("Move the stick so Vertex can see live input first.".to_owned());
                    ModalAction::None
                }
            },
        }
    }
}

#[derive(Debug, Clone)]
pub enum ModalAction {
    None,
    Cancel,
    Save {
        device_key: String,
        calibration: GamepadCalibration,
    },
}

fn step_description(step: CalibrationStep) -> &'static str {
    match step {
        CalibrationStep::Neutral => {
            "Leave the left stick untouched, then capture its neutral resting position."
        }
        CalibrationStep::Up => "Push the left stick upward and capture it.",
        CalibrationStep::Down => "Push the left stick downward and capture it.",
        CalibrationStep::Left => "Push the left stick left and capture it.",
        CalibrationStep::Right => "Push the left stick right and capture it.",
        CalibrationStep::Review => {
            "Review complete. Save the calibration to use this controller automatically next time."
        }
    }
}

fn capture_step(
    state: &mut GamepadCalibrationState,
    step: CalibrationStep,
    sample: GamepadStickSample,
) {
    state.error = None;
    match step {
        CalibrationStep::Neutral => {
            state.neutral = Some(sample);
            state.step = Some(CalibrationStep::Up);
        }
        CalibrationStep::Up => {
            state.up = Some(sample);
            state.step = Some(CalibrationStep::Down);
        }
        CalibrationStep::Down => {
            state.down = Some(sample);
            state.step = Some(CalibrationStep::Left);
        }
        CalibrationStep::Left => {
            state.left = Some(sample);
            state.step = Some(CalibrationStep::Right);
        }
        CalibrationStep::Right => {
            state.right = Some(sample);
            state.step = Some(CalibrationStep::Review);
        }
        CalibrationStep::Review => {}
    }
}

fn build_calibration(state: &GamepadCalibrationState) -> Option<GamepadCalibration> {
    let neutral = state.neutral?;
    let up = state.up?;
    let down = state.down?;
    let left = state.left?;
    let right = state.right?;

    let right_dx = right.x - neutral.x;
    let left_dx = left.x - neutral.x;
    let up_dy = up.y - neutral.y;
    let down_dy = down.y - neutral.y;

    if right_dx.abs() < 0.2 || left_dx.abs() < 0.2 || up_dy.abs() < 0.2 || down_dy.abs() < 0.2 {
        return None;
    }

    let x_extent = right_dx.abs().min(left_dx.abs());
    let y_extent = up_dy.abs().min(down_dy.abs());
    let deadzone_x = (x_extent * 0.3).clamp(0.12, 0.45);
    let deadzone_y = (y_extent * 0.3).clamp(0.12, 0.45);
    let threshold_x = (x_extent * 0.65).clamp(deadzone_x + 0.08, 0.92);
    let threshold_y = (y_extent * 0.65).clamp(deadzone_y + 0.08, 0.92);

    let mut calibration = GamepadCalibration {
        center_x: neutral.x,
        center_y: neutral.y,
        deadzone_x,
        deadzone_y,
        threshold_x,
        threshold_y,
        x_forward_sign: if right_dx >= 0.0 { 1 } else { -1 },
        y_backward_sign: if up_dy >= 0.0 { 1 } else { -1 },
    };
    calibration.normalize();
    Some(calibration)
}
