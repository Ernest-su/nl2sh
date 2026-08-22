use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use std::time::Duration;
pub fn next() -> Result<Option<Event>> {
    if event::poll(Duration::from_millis(100))? {
        Ok(Some(event::read()?))
    } else {
        Ok(None)
    }
}

#[derive(Default)]
pub(super) struct FragmentedArrowFilter {
    state: ArrowSequenceState,
}

#[derive(Default)]
enum ArrowSequenceState {
    #[default]
    Idle,
    Escape,
    Introducer,
}

impl FragmentedArrowFilter {
    pub(super) fn normalize(&mut self, mut key: KeyEvent) -> Option<KeyEvent> {
        match (&self.state, key.code) {
            (_, KeyCode::Esc) => {
                self.state = ArrowSequenceState::Escape;
                None
            }
            (ArrowSequenceState::Escape, KeyCode::Char('[' | 'O')) => {
                self.state = ArrowSequenceState::Introducer;
                None
            }
            (ArrowSequenceState::Introducer, KeyCode::Char(character)) => {
                let code = match character {
                    'A' => Some(KeyCode::Up),
                    'B' => Some(KeyCode::Down),
                    'C' => Some(KeyCode::Right),
                    'D' => Some(KeyCode::Left),
                    'H' => Some(KeyCode::Home),
                    'F' => Some(KeyCode::End),
                    _ => None,
                };
                self.reset();
                if let Some(code) = code {
                    key.code = code;
                }
                Some(key)
            }
            _ => {
                self.reset();
                Some(key)
            }
        }
    }

    pub(super) fn reset(&mut self) {
        self.state = ArrowSequenceState::Idle;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    #[test]
    fn reconstructs_fragmented_csi_and_ss3_arrows() {
        let mut filter = FragmentedArrowFilter::default();
        for introducer in ['[', 'O'] {
            assert!(filter
                .normalize(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
                .is_none());
            assert!(filter
                .normalize(KeyEvent::new(KeyCode::Char(introducer), KeyModifiers::NONE,))
                .is_none());
            let key = filter
                .normalize(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::NONE))
                .map(|key| key.code);
            assert_eq!(key, Some(KeyCode::Up));
        }
    }

    #[test]
    fn preserves_unrelated_characters() {
        let mut filter = FragmentedArrowFilter::default();
        let key = filter
            .normalize(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE))
            .map(|key| key.code);
        assert_eq!(key, Some(KeyCode::Char('x')));
    }
}
