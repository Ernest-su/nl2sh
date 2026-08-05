use anyhow::Result;
use crossterm::event::{self, Event};
use std::time::Duration;
pub fn next() -> Result<Option<Event>> {
    if event::poll(Duration::from_millis(100))? {
        Ok(Some(event::read()?))
    } else {
        Ok(None)
    }
}
