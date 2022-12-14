use crate::control::{ControlledInterpreter, ControlledToInterpreter, FrameInfo, InterpreterState};
use crate::hooks::{FurtherHooks, InterpreterHook};
use crate::key::Keys;
use crate::Display;
use getset::{Getters, MutGetters};
use log::{debug, info, trace, warn};
use std::marker::PhantomData;
use std::time::Duration;

#[derive(Debug, Getters, MutGetters)]
#[getset(get = "pub", get_mut = "pub")]
pub struct Interpreter<I: ControlledInterpreter> {
    inner: I,
    buzzer_active: bool,
    step_frequency: u32,
    internal_frequency_scale: Option<f32>,
    sixty_hertz_progress: Duration,
    state: InterpreterState,
    #[getset(skip)]
    hooks: Vec<Box<dyn InterpreterHook<I>>>,
}

impl<T: ControlledInterpreter> Interpreter<T> {
    pub fn step(&mut self, keys: Keys) -> Option<Display> {
        self.hook_pre_cycle();
        let keys = self.hook_map_keys(self.state, keys);
        match self.state {
            InterpreterState::Normal => {}
            InterpreterState::Held => {
                todo!("Check for resume")
            }
            InterpreterState::WaitForKey(reg) => {
                if keys.pressed() {
                    let parsed_keys = keys.one_key();
                    if let Some(key) = parsed_keys {
                        info!("Key pressed, continuing!");
                        self.state = InterpreterState::Normal;
                        self.inner.set_register(reg, key);
                    } else {
                        warn!("Multiple keys pressed at once, not continuing!");
                        return None;
                    }
                } else {
                    return None;
                }
            }
            InterpreterState::BusyWaiting => return None,
        }
        trace!("Beginning step.");
        let mut frame_info = FrameInfo::empty();

        // TODO: Hook for RTC registers
        let internal_frequency = self.internal_frequency_scale.unwrap_or(1.);
        let more_progress =
            Duration::from_secs_f32(self.speed().as_secs_f32() * internal_frequency);
        self.sixty_hertz_progress += more_progress;
        while self.sixty_hertz_progress.as_secs_f32() >= 1. / 60. {
            self.sixty_hertz_progress -= Duration::from_secs_f32(1. / 60.);
            if self.inner.timer_tick_60hz().buzzer_active() {
                frame_info.set_buzzer(true);
            } else {
                frame_info.set_buzzer(false);
            }
        }
        self.hook_before_step(&mut frame_info);
        self.inner.step(keys, &mut frame_info);
        trace!("Step complete!");
        self.hook_after_step(&mut frame_info);

        let FrameInfo {
            screen_modified,
            buzzer_change_state,
            entered_busywait,
            wait_for_key,
        } = frame_info;

        if let Some(reg) = wait_for_key {
            self.state = InterpreterState::WaitForKey(reg);
            info!("Waiting to store next keypress in {:?}", reg);
        }

        if entered_busywait {
            self.state = InterpreterState::BusyWaiting;
        }

        if let Some(buzzer) = buzzer_change_state {
            self.buzzer_active = buzzer;
        }
        if screen_modified {
            debug!("Screen has been updated.");
            self.hook_post_cycle();
            return Some(*self.inner.display());
        }
        self.hook_post_cycle();
        None
    }

    pub fn speed(&self) -> Duration {
        Duration::from_secs_f32(1. / (self.step_frequency as f32))
    }
}

impl<T: ControlledToInterpreter> Interpreter<T> {
    pub fn new(from: T) -> Self {
        Self {
            inner: from,
            buzzer_active: false,
            step_frequency: 8,
            internal_frequency_scale: None,
            sixty_hertz_progress: Duration::ZERO,
            state: InterpreterState::Normal,
            hooks: vec![],
        }
    }
}

impl<T: ControlledToInterpreter> Interpreter<T> {
    fn hook_pre_cycle(&mut self) {
        for hook in &mut self.hooks {
            hook.pre_cycle(&mut self.state);
        }
    }

    fn hook_before_step(&mut self, frame: &mut FrameInfo) {
        for hook in &mut self.hooks {
            hook.before_step(&mut self.inner, frame);
        }
    }

    fn hook_after_step(&mut self, frame: &mut FrameInfo) {
        for hook in &mut self.hooks {
            hook.after_step(&mut self.inner, frame);
        }
    }

    fn hook_post_cycle(&mut self) {
        for hook in &mut self.hooks {
            hook.post_cycle(&mut self.state);
        }
    }

    fn hook_map_keys(&mut self, state: InterpreterState, mut keys: Keys) -> Keys {
        for hook in &mut self.hooks {
            let ret = hook.get_keys(state, &self.inner, keys);
            if let Some(new_keys) = ret.item {
                keys = new_keys;
            }
            if ret.behaviour == FurtherHooks::Stop {
                break;
            }
        }
        keys
    }
}

impl<T: ControlledToInterpreter> Interpreter<T> {
    fn new_with_hooks(from: T, hooks: Vec<Box<dyn InterpreterHook<T>>>) -> Self {
        Self {
            hooks,
            ..Interpreter::new(from)
        }
    }

    pub fn with_frequency(mut self, frequency: u32) -> Self {
        self.step_frequency = frequency;
        self
    }

    pub fn with_simulated_frequency(mut self, frequency_scale: Option<f32>) -> Self {
        self.internal_frequency_scale = frequency_scale;
        self
    }
}

impl<T: ControlledInterpreter> Interpreter<T> {
    pub fn builder() -> InterpreterBuilder<T> {
        InterpreterBuilder::new()
    }
}

#[derive(Debug)]
pub struct InterpreterBuilder<T> {
    __phantom_interpreter: PhantomData<T>,
    hooks: Vec<Box<dyn InterpreterHook<T>>>,
}

impl<T: ControlledInterpreter> InterpreterBuilder<T> {
    pub fn extend_with<N: InterpreterHook<T> + 'static>(
        self,
        with: N,
    ) -> InterpreterBuilder<T> {
        let Self { mut hooks, .. } = self;
        hooks.push(Box::new(with));
        InterpreterBuilder {
            hooks,
            __phantom_interpreter: Default::default(),
        }
    }

    pub fn extend<N: InterpreterHook<T> + Default + 'static>(self) -> InterpreterBuilder<T> {
        let Self { mut hooks, .. } = self;
        hooks.push(Box::new(N::default()));
        InterpreterBuilder {
            hooks,
            __phantom_interpreter: Default::default(),
        }
    }

    pub fn build(self, with: T) -> Interpreter<T> {
        Interpreter::new_with_hooks(with, self.hooks)
    }
}

impl<T: ControlledInterpreter> InterpreterBuilder<T> {
    fn new() -> Self {
        Self {
            hooks: vec![],
            __phantom_interpreter: Default::default(),
        }
    }
}
