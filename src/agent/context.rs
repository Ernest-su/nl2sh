use crate::llm::{ConversationItem, ConversationMessage, Role};
/// Bounded text history that removes only complete interaction units.
pub struct ConversationContext {
    system: ConversationItem,
    turns: Vec<Vec<ConversationItem>>,
    max: usize,
}
impl ConversationContext {
    /// Creates a context whose system instruction is never truncated.
    pub fn new(system: impl Into<String>, max: usize) -> Self {
        Self {
            system: ConversationItem::Message(ConversationMessage::new(Role::System, system)),
            turns: Vec::new(),
            max,
        }
    }
    /// Adds one complete user/assistant interaction and evicts oldest units.
    pub fn push_turn(&mut self, messages: Vec<ConversationItem>) {
        self.turns.push(messages);
        while self.turns.len() > self.max {
            self.turns.remove(0);
        }
    }
    /// Returns the system message followed by retained complete interactions.
    pub fn items(&self) -> Vec<ConversationItem> {
        std::iter::once(self.system.clone())
            .chain(self.turns.iter().flatten().cloned())
            .collect()
    }
}
